"""Export reference DDL from the pinned WP builder, without connecting to a DB."""

import argparse
from datetime import datetime, timezone
import json
from pathlib import Path
import re
import subprocess

import wordpress_reference as reference


def main(php):
    source_root=reference.REFERENCE/f"wordpress-{reference.VERSION}"
    archive=reference.REFERENCE/f"wordpress-{reference.VERSION}.tar.gz"
    if reference.digest_file(archive,"sha1")!=reference.EXPECTED_SHA1:raise ValueError("reference archive changed")
    manifest=json.loads((source_root/reference.MANIFEST_NAME).read_text(encoding="utf-8"))
    source_path="wp-admin/includes/schema.php"
    expected=next(row["sha256"] for row in manifest["files"] if row["path"]==source_path)
    source=source_root/source_path
    if reference.digest_file(source)!=expected:raise ValueError("reference schema builder changed")
    output=subprocess.run([str(php),"-n",str(reference.ROOT/"scripts/wp_schema_export.php"),str(source_root)],stdout=subprocess.PIPE,stderr=subprocess.PIPE,check=True,timeout=30)
    if output.stderr:raise ValueError("unexpected schema builder diagnostics")
    data=json.loads(output.stdout)
    if len(data["tables"])!=12 or len(set(data["tables"]))!=12:raise ValueError("unexpected single-site fixture table set")
    target=reference.ROOT/"compatibility/schema";target.mkdir(exist_ok=True)
    sql=target/f"wordpress-{reference.VERSION}-single-site.sql"
    sql.write_bytes(data["sql"].encode("utf-8"))
    lines=source.read_text(encoding="utf-8").splitlines()
    metadata={"format_version":1,"scope":"reference_DDL_only_not_installation_or_DB_compatibility","generated_at_utc":datetime.now(timezone.utc).isoformat(),"wordpress_version":reference.VERSION,"archive_sha256":reference.digest_file(archive),"php_version":data["php_version"],"context":data["context"],"tables":data["tables"],"sql_sha256":reference.digest_file(sql),"source_path":source_path,"source_sha256":expected,"source_line":next(i+1 for i,line in enumerate(lines) if re.match(r'^function wp_get_db_schema\(',line)),"tool_files":{name:reference.digest_file(reference.ROOT/name) for name in ["scripts/wp_schema_export.php","scripts/reference_functions.php","scripts/export_wordpress_schema.py"]},"limitations":["Single-site/default-prefix fixture only; custom physical placement and multisite are not established.","Charset/collation are explicit fixture inputs, not a product support decision.","No database connection, schema creation, or data migration occurs in this exporter."]}
    (target/"reference.json").write_text(json.dumps(metadata,indent=2)+"\n",encoding="utf-8")
    print(f"Exported {len(data['tables'])} single-site reference tables: {sql.relative_to(reference.ROOT)}")


if __name__=="__main__":
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument("--php",type=Path,default=reference.REFERENCE/"php-8.5.12/php.exe");options=parser.parse_args()
    try:main(options.php.resolve())
    except subprocess.CalledProcessError as error:parser.exit(1,f"Schema export command failed: {error}\n{(error.stderr or b'').decode(errors='replace')}\n{(error.stdout or b'').decode(errors='replace')}\n")
    except (OSError,ValueError,KeyError,StopIteration,subprocess.TimeoutExpired) as error:parser.exit(1,f"Schema export failed: {error}\n")
