"""Generate and cross-verify finite public dummies; never compare random hashes."""
from datetime import datetime, timezone
import json
import os
from pathlib import Path

import check_password_generation_evidence as evidence
from owned_probe import run_captured_probe, reap_owned_probes

ROOT = evidence.ROOT

def run(command, request, php=False):
    wire = evidence.canonical(request)
    if len(wire) > 65536: raise ValueError('probe input bound')
    pid, completed = run_captured_probe(command, os.environ.copy(), timeout=120, input_bytes=wire)
    if completed.returncode != 0 or completed.stderr or len(completed.stdout) > 65536:
        raise ValueError('password generation child failed or exceeded output bound')
    output = completed.stdout
    if not output.endswith(b'\n') or b'\n' in output[:-1]: raise ValueError('probe must return one complete line')
    value = json.loads(output.decode('utf-8'), object_pairs_hook=evidence.unique_object)
    fields = ['process_pid', 'results']
    if php: fields += ['php_version', 'globals_restored', 'actions_unchanged', 'restoration_states']
    evidence.fields(value, fields)
    evidence.positive_pid(value['process_pid'])
    if value['process_pid'] != pid or type(value['results']) is not list or len(value['results']) != len(request['items']):
        raise ValueError('probe identity or result count differs')
    if php and (value['php_version'] != '8.5.12' or value['globals_restored'] is not True
                or value['actions_unchanged'] is not True
                or value['restoration_states'] != ['absent', 'null', 'sentinel', 'empty']):
        raise ValueError('PHP version/context restoration differs')
    return value, {'owned_pid': pid, 'reported_pid': value['process_pid'], 'exit_code': completed.returncode,
                   'stderr_empty': completed.stderr == b''}

def build():
    pid, result = run_captured_probe(['cargo', 'build', '-p', 'recastpress-wp', '--example',
        'password_generation_probe', '--offline', '--locked', '--message-format=json'], os.environ.copy(), timeout=120)
    if result.returncode != 0 or len(result.stdout) > 4 * 1024 * 1024:
        raise ValueError('password generation probe build failed')
    paths = []
    for line in result.stdout.splitlines():
        row = json.loads(line, object_pairs_hook=evidence.unique_object)
        if (row.get('reason') == 'compiler-artifact' and row.get('target', {}).get('name') == 'password_generation_probe'
                and row.get('target', {}).get('kind') == ['example'] and row.get('executable')):
            paths.append(Path(row['executable']).resolve())
    expected = (ROOT / evidence.binary_paths()['rust']).resolve()
    if paths != [expected] or not expected.is_file(): raise ValueError('current Cargo probe executable is ambiguous')
    return expected

def compare():
    evidence.verify_pinned()
    before = {path: evidence.digest(ROOT / path) for path in sorted(evidence.tracked_files())}
    binary = build()
    binary_hashes = {name: {'path': path, 'sha256': evidence.digest(ROOT / path)}
                     for name, path in evidence.binary_paths().items()}
    php = [str(ROOT / evidence.binary_paths()['php']), '-n', '-d', 'memory_limit=128M',
           str(ROOT / 'scripts/password_generation_oracle.php'), str(ROOT / '.reference/wordpress-7.1.3')]
    samples = evidence.cases()
    native_items = [{'password_hex': password.hex(), 'maximum': 12} for _, password in samples]
    native_items += [{'password_hex': b'public-dummy'.hex(), 'maximum': 4},
                     {'password_hex': (b'x' * 4097).hex(), 'maximum': 4}]
    native, native_run = run([str(binary)], {'operation': 'generate', 'items': native_items})
    actual, actual_run = run(php, {'operation': 'generate', 'items': [
        {'password_hex': password.hex()} for _, password in samples]}, php=True)
    rust_verify, php_verify, records = [], [], []
    for (name, password), ours, theirs in zip(samples, native['results'][:len(samples)], actual['results'], strict=True):
        evidence.fields(ours, ['hash_hex'])
        evidence.fields(theirs, ['hash_hex', 'algorithm_calls', 'options_calls'])
        record = {'name': name, 'native_hash_hex': ours['hash_hex'], 'php_hash_hex': theirs['hash_hex'],
                  'algorithm_calls': theirs['algorithm_calls'], 'options_calls': theirs['options_calls'],
                  'globals_restored': actual['globals_restored'], 'actions_unchanged': actual['actions_unchanged'],
                  'php_checks': [], 'native_checks': [], 'check_calls': []}
        records.append(record)
        if len(password) > 4096: continue
        evidence.hash_shape(ours['hash_hex']); evidence.hash_shape(theirs['hash_hex'])
        for candidate in (evidence.trim6(password), password, b'wrong-public-generation-dummy'):
            php_verify.append({'password_hex': candidate.hex(), 'hash_hex': ours['hash_hex']})
            rust_verify.append({'password_hex': candidate.hex(), 'hash_hex': theirs['hash_hex'], 'maximum': 12})
    checked_php, verify_php_run = run(php, {'operation': 'verify', 'items': php_verify}, php=True)
    checked_native, verify_native_run = run([str(binary)], {'operation': 'verify', 'items': rust_verify})
    offset = 0
    for record, (_, password) in zip(records, samples, strict=True):
        if len(password) > 4096: continue
        for theirs, ours in zip(checked_php['results'][offset:offset+3], checked_native['results'][offset:offset+3], strict=True):
            evidence.fields(theirs, ['verified', 'check_calls']); evidence.fields(ours, ['verified'])
            record['php_checks'].append(theirs['verified']); record['native_checks'].append(ours['verified'])
            record['check_calls'].append(theirs['check_calls'])
        offset += 3
    low, long = native['results'][-2:]
    evidence.fields(low, ['error']); evidence.fields(long, ['hash_hex'])
    record = {'format_version': 1, 'scope': 'public_dummy_wp_password_generation_cross_verification',
              'generated_at_utc': datetime.now(timezone.utc).isoformat(), 'profile': evidence.PROFILE,
              'corpus_sha256': evidence.corpus_hash(), 'runs': {'rust_generate': native_run, 'php_generate': actual_run,
              'php_verify': verify_php_run, 'rust_verify': verify_native_run}, 'cases': records,
              'host_refusals': {'within_max4': low['error'], 'over_max4': long['hash_hex']},
              'tracked_files': before, 'binaries': binary_hashes, 'limitations': evidence.LIMITATIONS}
    reap_owned_probes()
    evidence.validate_record(record)
    wire = json.dumps(record, indent=2, ensure_ascii=True) + '\n'
    if len(wire.encode('utf-8')) > evidence.MAX_JSON: raise ValueError('final evidence bound')
    # No artifact is opened until every child/result/current hash has passed.
    evidence.OUTPUT.write_text(wire, encoding='utf-8', newline='\n')
    print(f'Cross-verified {len(samples)} public dummy generation cases; separate relation evidence saved')

def main():
    original = None
    try:
        compare()
    except BaseException as error:
        original = error
        raise
    finally:
        if original is not None:
            try:
                reap_owned_probes()
            except BaseException as cleanup:
                original.add_note('Owned password probe cleanup retry failed: ' + type(cleanup).__name__)

if __name__ == '__main__':
    main()
