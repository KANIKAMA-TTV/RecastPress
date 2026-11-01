"""Run owned finite record-backend stage relations, never real authentication."""
from datetime import datetime, timezone
import json
import os
from pathlib import Path

import check_credential_stage_evidence as evidence
from owned_probe import run_captured_probe, reap_owned_probes

def build():
    _,run=run_captured_probe(['cargo','build','-p','recastpress-wp','--example','credential_stage_probe',
        '--offline','--locked','--message-format=json'],os.environ.copy(),timeout=120)
    if run.returncode!=0 or len(run.stdout)>4*1024*1024:raise ValueError('credential stage probe build failed')
    found=[]
    for line in run.stdout.splitlines():
        row=json.loads(line,object_pairs_hook=evidence.common.unique_object)
        if(row.get('reason')=='compiler-artifact' and row.get('target',{}).get('name')=='credential_stage_probe'
            and row.get('target',{}).get('kind')==['example']and row.get('executable')):
            found.append(Path(row['executable']).resolve())
    expected=(evidence.ROOT/evidence.binary_paths()['rust']).resolve()
    if found!=[expected]or not expected.is_file():raise ValueError('credential stage executable ambiguous')
    return expected

def run(command,request,php=False):
    wire=evidence.common.canonical(request)
    if len(wire)>1048576:raise ValueError('credential stage input bound')
    pid,result=run_captured_probe(command,os.environ.copy(),timeout=60,input_bytes=wire)
    if result.returncode!=0 or result.stderr or len(result.stdout)>131072:raise ValueError('credential stage child refused or output bound')
    if not result.stdout.endswith(b'\n')or b'\n'in result.stdout[:-1]:raise ValueError('credential stage output must be one line')
    value=json.loads(result.stdout.decode('utf-8'),object_pairs_hook=evidence.common.unique_object)
    fields=['process_pid','results']+(['php_version','globals_restored','actions_restored','restoration_states','db_writes']if php else [])
    evidence.common.fields(value,fields);evidence.common.positive_pid(value['process_pid'])
    if value['process_pid']!=pid or type(value['results'])is not list or len(value['results'])!=len(request['items']):raise ValueError('credential stage PID/coverage differs')
    return value,{'owned_pid':pid,'reported_pid':value['process_pid'],'exit_code':result.returncode,'stderr_empty':result.stderr==b''}

def compare():
    evidence.verify_pinned()
    hashes={path:evidence.common.digest(evidence.ROOT/path)for path in sorted(evidence.tracked_files())}
    executable=build()
    binaries={name:{'path':path,'sha256':evidence.common.digest(evidence.ROOT/path)}for name,path in evidence.binary_paths().items()}
    cases=evidence.cases();request={'items':[case['request']for case in cases]}
    native,native_run=run([str(executable)],request)
    php,php_run=run([str(evidence.ROOT/evidence.binary_paths()['php']),'-n','-d','memory_limit=128M',
        str(evidence.ROOT/'scripts/credential_stage_oracle.php'),str(evidence.ROOT/'.reference/wordpress-7.1.3')],request,php=True)
    record={'format_version':1,'scope':'credential_stage_public_dummy_relations',
        'generated_at_utc':datetime.now(timezone.utc).isoformat(),'profile':evidence.PROFILE,
        'corpus_sha256':evidence.corpus_hash(),'runs':{'rust':native_run,'php':php_run},
        'oracle':{key:php[key]for key in ('php_version','globals_restored','actions_restored','restoration_states','db_writes')},
        'cases':[{'id':case['id'],'native':ours,'php':theirs}for case,ours,theirs in zip(cases,native['results'],php['results'],strict=True)],
        'tracked_files':hashes,'binaries':binaries,'limitations':evidence.LIMITATIONS}
    reap_owned_probes()
    evidence.validate_record(record)
    wire=json.dumps(record,ensure_ascii=True,indent=2)+'\n'
    if len(wire.encode('utf-8'))>evidence.MAX_JSON:raise ValueError('credential stage evidence bound')
    evidence.OUTPUT.write_text(wire,encoding='utf-8',newline='\n')
    print(f'Credential stage relations: {len(cases)}; actual callback pairs: {sum(case["actual"]for case in cases)}; refusals separate')

def main():
    original=None
    try:compare()
    except BaseException as error:
        original=error
        raise
    finally:
        if original is not None:
            try:reap_owned_probes()
            except BaseException as cleanup:original.add_note('Owned credential stage cleanup retry failed: '+type(cleanup).__name__)

if __name__=='__main__':main()
