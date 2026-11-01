"""Verify a pinned MySQL development ZIP with the documented release-key fingerprint."""

import json
import os
from pathlib import Path
import subprocess
import urllib.parse
import urllib.request
import uuid

from pinned_zip_tool import ensure_tool
import wordpress_reference as reference

VERSION="8.4.13"
FILENAME=f"mysql-{VERSION}-winx64.zip"
URL="https://cdn.mysql.com/Downloads/MySQL-8.4/"+FILENAME
KEY_URL="https://repo.mysql.com/RPM-GPG-KEY-mysql-2025"
FINGERPRINT="BCA43417C3B485DD128EC6D4B7B3B788A8D3785C"
GPG=Path("C:/Program Files/Git/usr/bin/gpg.exe")
MAX_DOWNLOAD=384*1024*1024

def gpg_path(path):
    path=Path(path).resolve().as_posix()
    if len(path)>2 and path[1]==':':return '/'+path[0].lower()+path[2:]
    return path


def download(url,target,limit):
    partial=reference.REFERENCE/f"mysql-download-{uuid.uuid4().hex}.part"
    with urllib.request.urlopen(url,timeout=30) as response,partial.open("xb") as output:
        final=urllib.parse.urlsplit(response.url)
        if final.scheme!="https" or final.hostname!=urllib.parse.urlsplit(url).hostname:raise ValueError("unexpected MySQL download redirect")
        size=0
        while chunk:=response.read(1024*1024):
            size+=len(chunk)
            if size>limit:raise ValueError("MySQL download exceeds limit")
            output.write(chunk)
    partial.replace(target)


def main():
    if os.name!="nt" or not GPG.is_file():raise ValueError("this helper requires the existing Git for Windows GnuPG executable")
    reference.REFERENCE.mkdir(exist_ok=True)
    archive=reference.REFERENCE/FILENAME
    signature=reference.REFERENCE/(FILENAME+".asc")
    key=reference.REFERENCE/"mysql-release-key.asc"
    if not archive.exists():download(URL,archive,MAX_DOWNLOAD)
    if not signature.exists():download(URL+".asc",signature,65536)
    download(KEY_URL,key,65536)
    home=reference.REFERENCE/"mysql-gpg";home.mkdir(exist_ok=True)
    command=[str(GPG),"--homedir",gpg_path(home),"--batch","--no-tty","--no-autostart"]
    inspected=subprocess.run([*command,"--with-colons","--show-keys",gpg_path(key)],stdout=subprocess.PIPE,stderr=subprocess.PIPE,check=True,timeout=30,creationflags=subprocess.CREATE_NO_WINDOW)
    fingerprints=[line.split(":")[9] for line in inspected.stdout.decode().splitlines() if line.startswith("fpr:")]
    if not fingerprints or fingerprints[0]!=FINGERPRINT:raise ValueError("MySQL release key fingerprint differs from the official documented key")
    subprocess.run([*command,"--import",gpg_path(key)],stdout=subprocess.PIPE,stderr=subprocess.PIPE,check=True,timeout=30,creationflags=subprocess.CREATE_NO_WINDOW)
    verified=subprocess.run([*command,"--status-fd=1","--verify",gpg_path(signature),gpg_path(archive)],stdout=subprocess.PIPE,stderr=subprocess.PIPE,check=True,timeout=60,creationflags=subprocess.CREATE_NO_WINDOW)
    valid=[line.split()[2] for line in verified.stdout.decode().splitlines() if line.startswith("[GNUPG:] VALIDSIG ")]
    if FINGERPRINT not in valid:raise ValueError("MySQL archive has no valid signature from the pinned release key")
    digest=reference.digest_file(archive)
    target=ensure_tool(f"mysql-{VERSION}",URL,digest,strip_root=f"mysql-{VERSION}-winx64",max_download=MAX_DOWNLOAD,max_expanded=2*1024*1024*1024)
    manifest_path=target/"recastpress-tool.json"
    manifest=json.loads(manifest_path.read_text(encoding="utf-8"))
    manifest.update(signature_url=URL+".asc",signer_fingerprint=FINGERPRINT,release_key_url=KEY_URL,signature_sha256=reference.digest_file(signature),release_key_sha256=reference.digest_file(key))
    manifest_path.write_text(json.dumps(manifest,indent=2)+"\n",encoding="utf-8")
    print(f"Verified MySQL {VERSION} signature: {FINGERPRINT}; archive SHA-256 {digest}")
    print(target/"bin/mysqld.exe")


if __name__=="__main__":
    try:main()
    except subprocess.CalledProcessError as error:raise SystemExit("MySQL signature verification failed:\n"+(error.stderr or b'').decode(errors="replace"))
    except (OSError,ValueError,KeyError,subprocess.TimeoutExpired) as error:raise SystemExit(f"MySQL development setup failed: {error}")
