"""Actual pinned WP legacy password verification, separate from primitive counts."""
from datetime import datetime, timezone
import json
import os
from pathlib import Path

import check_legacy_password_evidence as evidence
from owned_probe import run_captured_probe, reap_owned_probes

def build():
    _, result = run_captured_probe(['cargo', 'build', '-p', 'recastpress-wp', '--example',
        'legacy_password_probe', '--offline', '--locked', '--message-format=json'], os.environ.copy(), timeout=120)
    if result.returncode != 0 or len(result.stdout) > 4*1024*1024: raise ValueError('legacy probe build failed')
    paths = []
    for line in result.stdout.splitlines():
        row = json.loads(line, object_pairs_hook=evidence.common.unique_object)
        if (row.get('reason') == 'compiler-artifact' and row.get('target', {}).get('name') == 'legacy_password_probe'
                and row.get('target', {}).get('kind') == ['example'] and row.get('executable')):
            paths.append(Path(row['executable']).resolve())
    expected = (evidence.ROOT/evidence.binary_paths()['rust']).resolve()
    if paths != [expected] or not expected.is_file(): raise ValueError('legacy current executable ambiguous')
    return expected

def run(command, request, php=False):
    wire = evidence.common.canonical(request)
    if len(wire) > 1024*1024: raise ValueError('legacy input bound')
    pid, result = run_captured_probe(command, os.environ.copy(), timeout=60, input_bytes=wire)
    if result.returncode != 0 or result.stderr or len(result.stdout) > 65536:
        raise ValueError('legacy child diagnostics or output bound')
    if not result.stdout.endswith(b'\n') or b'\n' in result.stdout[:-1]: raise ValueError('legacy output must be one line')
    value = json.loads(result.stdout.decode('utf-8'), object_pairs_hook=evidence.common.unique_object)
    fields = ['process_pid', 'results'] + (['php_version', 'globals_restored', 'actions_unchanged', 'restoration_states'] if php else [])
    evidence.common.fields(value, fields); evidence.common.positive_pid(value['process_pid'])
    if value['process_pid'] != pid or type(value['results']) is not list or len(value['results']) != len(request['items']):
        raise ValueError('legacy PID/result count differs')
    return value, {'owned_pid': pid, 'reported_pid': value['process_pid'], 'exit_code': result.returncode,
                   'stderr_empty': result.stderr == b''}

def compare():
    evidence.verify_pinned()
    hashes = {path: evidence.common.digest(evidence.ROOT/path) for path in sorted(evidence.tracked_files())}
    executable = build()
    binaries = {name: {'path': path, 'sha256': evidence.common.digest(evidence.ROOT/path)}
                for name, path in evidence.binary_paths().items()}
    cases = evidence.cases()
    request = {'items': [case['request'] for case in cases]}
    native, native_run = run([str(executable)], request)
    php, php_run = run([str(evidence.ROOT/evidence.binary_paths()['php']), '-n', '-d', 'memory_limit=128M',
        str(evidence.ROOT/'scripts/legacy_password_oracle.php'), str(evidence.ROOT/'.reference/wordpress-7.1.3')], request, php=True)
    record = {'format_version': 1, 'scope': 'public_dummy_legacy_password_verification',
        'generated_at_utc': datetime.now(timezone.utc).isoformat(), 'profile': evidence.PROFILE,
        'corpus_sha256': evidence.corpus_hash(), 'runs': {'rust': native_run, 'php': php_run},
        'oracle': {key: php[key] for key in ('php_version', 'globals_restored', 'actions_unchanged', 'restoration_states')},
        'cases': [{'id': case['id'], 'native': ours, 'php': theirs}
                  for case, ours, theirs in zip(cases, native['results'], php['results'], strict=True)],
        'tracked_files': hashes, 'binaries': binaries, 'limitations': evidence.LIMITATIONS}
    reap_owned_probes()
    evidence.validate_record(record)
    wire = json.dumps(record, ensure_ascii=True, indent=2)+'\n'
    if len(wire.encode('utf-8')) > evidence.MAX_JSON: raise ValueError('legacy evidence bound')
    evidence.OUTPUT.write_text(wire, encoding='utf-8', newline='\n')
    print(f'Legacy public dummy cases: {len(cases)}; actual WP calls: {sum(case["actual"] for case in cases)}; explicit refusals recorded separately')

def main():
    original = None
    try:
        compare()
    except BaseException as error:
        original = error
        raise
    finally:
        if original is not None:
            try: reap_owned_probes()
            except BaseException as cleanup:
                original.add_note('Owned legacy probe cleanup retry failed: '+type(cleanup).__name__)

if __name__ == '__main__': main()
