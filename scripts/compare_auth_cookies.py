"""Observe owned public-dummy WP cookie relations, with unchanged PHP time()."""
from datetime import datetime, timezone
import copy
import json
import os
from pathlib import Path
import time

import check_auth_cookie_evidence as evidence
from owned_probe import run_captured_probe, reap_owned_probes


def run(command, wire=None, timeout=60):
    if wire is not None and (type(wire) is not bytes or len(wire) > 8 * 1024 * 1024 + 1):
        raise ValueError('cookie request bound')
    pid, result = run_captured_probe(command, os.environ.copy(), timeout=timeout, input_bytes=wire)
    if len(result.stdout) > 4 * 1024 * 1024 or len(result.stderr) > 1024 * 1024:
        raise ValueError('cookie postcapture output bound')
    return pid, result


def build():
    root = evidence.ROOT
    _, result = run(['cargo', 'build', '-p', 'recastpress-wp', '--example', 'auth_cookie_verify_probe',
                     '--offline', '--locked', '--message-format=json'], timeout=180)
    (root / 'target/auth-cookie-verify-build.jsonl').write_bytes(result.stdout)
    (root / 'target/auth-cookie-verify-build-stderr.log').write_bytes(result.stderr)
    if result.returncode != 0:
        raise ValueError('cookie probe build failed')
    manifest = (root / 'crates/recastpress-wp/Cargo.toml').resolve()
    source = (root / 'crates/recastpress-wp/examples/auth_cookie_verify_probe.rs').resolve()
    found, finished = [], []
    for line in result.stdout.splitlines():
        row = json.loads(line, object_pairs_hook=evidence.common.unique_object)
        if row.get('reason') == 'build-finished':
            finished.append(row.get('success') is True)
        if (row.get('reason') == 'compiler-artifact'
                and Path(row.get('manifest_path', '')).resolve() == manifest
                and row.get('target', {}).get('name') == 'auth_cookie_verify_probe'
                and row.get('target', {}).get('kind') == ['example']
                and Path(row.get('target', {}).get('src_path', '')).resolve() == source
                and row.get('executable')):
            found.append(Path(row['executable']).resolve())
    expected = (root / evidence.binary_paths()['rust']).resolve()
    if found != [expected] or finished != [True] or not expected.is_file():
        raise ValueError('cookie probe artifact ambiguous')
    return expected


def observe(command, requests, php=False):
    wire = json.dumps({'cases': requests}, ensure_ascii=True, separators=(',', ':')).encode('ascii')
    if php:
        wire += b'\n'
    pid, result = run(command, wire)
    if (result.returncode != 0 or result.stderr or len(result.stdout) > 1024 * 1024
            or not result.stdout.endswith(b'\n') or b'\n' in result.stdout[:-1]):
        raise ValueError('cookie child refused observation')
    value = json.loads(result.stdout, object_pairs_hook=evidence.common.unique_object,
                       parse_constant=lambda _: (_ for _ in ()).throw(ValueError('nonfinite JSON')))
    evidence.common.fields(value, ['format_version', 'process_pid', 'cases'] + (['profile'] if php else []))
    evidence.common.positive_pid(value['process_pid'])
    if not evidence.common.exact(value['format_version'], 1) or not evidence.common.exact(value['process_pid'], pid):
        raise ValueError('cookie reported PID differs')
    return value, {'owned_pid': pid, 'reported_pid': value['process_pid'], 'exit_code': result.returncode,
                    'stderr_empty': result.stderr == b''}


def refusal_controls(binary, valid_case):
    envelope = {'cases': [valid_case]}
    compact = lambda value: json.dumps(value, ensure_ascii=True, separators=(',', ':')).encode('ascii')
    cases_wire = compact([valid_case])
    invalid = copy.deepcopy(valid_case)
    invalid['id'] += 1
    invalid['sessions'] = [{'verifier': 'not-a-prepared-verifier', 'expiration': '1', 'legacy': False}]
    controls = (
        ([str(binary), 'PUBLIC-DUMMY-DO-NOT-ECHO'], compact(envelope)),
        ([str(binary)], b'{"cases":' + cases_wire + b',"cases":' + cases_wire + b'}'),
        ([str(binary)], b'{"cases":' + cases_wire + b',"unknown":true}'),
        ([str(binary)], compact({'cases': [valid_case, invalid]})),
        ([str(binary)], b' ' * (8 * 1024 * 1024 + 1)),
    )
    rows = []
    for name, (command, wire) in zip(evidence.CONTROL_NAMES, controls, strict=True):
        pid, result = run(command, wire)
        if result.returncode != 2 or result.stdout or result.stderr != b'prepared cookie proof refused\n':
            raise ValueError('cookie closed CLI control failed')
        rows.append({'id': name, 'owned_pid': pid, 'exit_code': 2, 'stdout_empty': True, 'fixed_opaque_stderr': True})
    return rows


def compare():
    evidence.verify_pinned()
    sources_before = {path: evidence.common.digest(evidence.ROOT / path) for path in sorted(evidence.tracked_files())}
    binary = build()
    binaries_before = {name: {'path': path, 'sha256': evidence.common.digest(evidence.ROOT / path)}
                       for name, path in evidence.binary_paths().items()}
    anchor = int(time.time())
    cases = evidence.corpus.cases(anchor)
    requests = [case['request_without_now'] for case in cases]
    php, php_run = observe([str(evidence.ROOT / evidence.binary_paths()['php']), '-n', '-d', 'memory_limit=128M',
                            str(evidence.ROOT / 'scripts/auth_cookie_oracle.php'), str(evidence.ROOT)], requests, php=True)
    evidence.validate_php_cases(php['cases'], cases, anchor)
    before_requests = [dict(request, now=str(row['clock_before'])) for request, row in zip(requests, php['cases'], strict=True)]
    after_requests = [dict(request, now=str(row['clock_after'])) for request, row in zip(requests, php['cases'], strict=True)]
    before, before_run = observe([str(binary)], before_requests)
    after, after_run = observe([str(binary)], after_requests)
    evidence.validate_native_cases(before['cases'], cases)
    evidence.validate_native_cases(after['cases'], cases)
    controls = refusal_controls(binary, before_requests[0])
    sources_after = {path: evidence.common.digest(evidence.ROOT / path) for path in sorted(evidence.tracked_files())}
    binaries_after = {name: {'path': path, 'sha256': evidence.common.digest(evidence.ROOT / path)}
                      for name, path in evidence.binary_paths().items()}
    evidence.verify_pinned()
    if sources_before != sources_after or binaries_before != binaries_after:
        raise ValueError('cookie sources or binaries changed during observation')
    reap_owned_probes()
    record = {
        'format_version': 1, 'scope': evidence.SCOPE, 'generated_at_utc': datetime.now(timezone.utc).isoformat(),
        'fixture_anchor': anchor, 'corpus_sha256': evidence.corpus_hash(cases), 'profile': php['profile'],
        'runs': {'php': php_run, 'native_before': before_run, 'native_after': after_run},
        'php': php['cases'], 'native_before': before['cases'], 'native_after': after['cases'],
        'closed_controls': controls, 'tracked_files': sources_before, 'binaries': binaries_before,
        'limitations': evidence.LIMITATIONS,
    }
    evidence.validate_record(record)
    wire = json.dumps(record, ensure_ascii=True, indent=2, allow_nan=False) + '\n'
    if len(wire.encode('ascii')) > evidence.MAX_JSON:
        raise ValueError('cookie evidence record bound')
    evidence.OUTPUT.write_text(wire, encoding='utf-8', newline='\n')
    print('Prepared cookie actual WP relations30; native endpoints60; closed controls5; no API count added')


def main():
    original = None
    try:
        compare()
    except BaseException as error:
        original = error
        raise
    finally:
        if original is None:
            reap_owned_probes()
        else:
            try:
                reap_owned_probes()
            except BaseException as cleanup:
                original.add_note('Owned cookie cleanup retry failed: ' + type(cleanup).__name__)


if __name__ == '__main__':
    main()
