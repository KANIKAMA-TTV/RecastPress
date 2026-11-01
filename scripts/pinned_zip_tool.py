"""Workspace-local ZIP tool extraction with pinned archive identity and bounded paths."""

import json
from pathlib import Path, PurePosixPath
import stat
import urllib.parse
import urllib.request
import uuid
import zipfile

import wordpress_reference as reference


def extract_zip(archive, destination, strip_root=None, max_expanded=1024*1024*1024):
    destination = Path(destination).resolve()
    destination.mkdir(parents=True, exist_ok=False)
    rows = []
    seen = set()
    with zipfile.ZipFile(archive) as source:
        members = source.infolist()
        if len(members) > 20000 or sum(member.file_size for member in members) > max_expanded:
            raise ValueError("tool archive exceeds expansion limits")
        for member in members:
            relative = reference.safe_relative("wordpress/" + member.filename)
            if strip_root:
                if not relative.parts or relative.parts[0] != strip_root:
                    raise ValueError("unexpected tool archive root")
                relative = PurePosixPath(*relative.parts[1:])
            if relative == PurePosixPath("."):
                if not member.is_dir(): raise ValueError("tool archive root is not a directory")
                continue
            name = relative.as_posix()
            if name.casefold() in seen or stat.S_ISLNK(member.external_attr >> 16):
                raise ValueError("duplicate/link entry in tool archive")
            seen.add(name.casefold())
            target = destination.joinpath(*relative.parts)
            if not target.resolve().is_relative_to(destination):
                raise ValueError("tool archive target escaped workspace staging")
            if member.is_dir():
                target.mkdir(parents=True, exist_ok=True)
                continue
            target.parent.mkdir(parents=True, exist_ok=True)
            with source.open(member) as data, target.open("xb") as output:
                while chunk := data.read(1024*1024): output.write(chunk)
            rows.append({"path":name,"sha256":reference.digest_file(target)})
    return rows


def ensure_tool(tool_id, url, expected_sha256, strip_root=None, max_download=128*1024*1024, max_expanded=1024*1024*1024):
    reference.REFERENCE.mkdir(exist_ok=True)
    filename = urllib.parse.urlsplit(url).path.rsplit("/",1)[1]
    if not filename or reference.safe_relative("wordpress/"+filename).name != filename:
        raise ValueError("invalid tool archive name")
    archive = reference.REFERENCE / filename
    target = reference.REFERENCE / tool_id
    if reference.safe_relative("wordpress/"+tool_id).name != tool_id:
        raise ValueError("invalid tool ID")
    if not archive.exists():
        partial = reference.REFERENCE / f"tool-{uuid.uuid4().hex}.part"
        with urllib.request.urlopen(url,timeout=30) as response,partial.open("xb") as output:
            final = urllib.parse.urlsplit(response.url)
            if final.scheme != "https" or final.hostname != urllib.parse.urlsplit(url).hostname:
                raise ValueError("unexpected tool archive redirect")
            total = 0
            while chunk := response.read(1024*1024):
                total += len(chunk)
                if total > max_download: raise ValueError("tool download exceeds size limit")
                output.write(chunk)
        if reference.digest_file(partial) != expected_sha256: raise ValueError("tool download checksum mismatch")
        partial.rename(archive)
    if archive.stat().st_size > max_download or reference.digest_file(archive) != expected_sha256:
        raise ValueError("tool archive differs from pinned checksum")
    if target.exists():
        manifest = json.loads((target/"recastpress-tool.json").read_text(encoding="utf-8"))
        if manifest["archive_sha256"] != expected_sha256: raise ValueError("tool manifest archive mismatch")
        for row in manifest["files"]:
            path = target.joinpath(*reference.safe_relative("wordpress/"+row["path"]).parts)
            if path.is_symlink() or reference.digest_file(path) != row["sha256"]: raise ValueError("installed tool file changed")
        return target
    staging = reference.REFERENCE / f"tool-extract-{uuid.uuid4().hex}"
    files = extract_zip(archive,staging,strip_root,max_expanded)
    (staging/"recastpress-tool.json").write_text(json.dumps({"tool_id":tool_id,"archive_url":url,"archive_sha256":expected_sha256,"files":files},indent=2)+"\n",encoding="utf-8")
    staging.rename(target)
    return target
