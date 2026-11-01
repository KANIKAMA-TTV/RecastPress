"""Finite callback relations with a record-only backend; never authentication proof."""
from datetime import datetime, timedelta
import hashlib
import json
import re

import check_password_generation_evidence as common
import check_legacy_password_evidence as legacy
import wp_bcrypt_profile as bcrypt

ROOT=common.ROOT
OUTPUT=ROOT/'compatibility/evidence/credential-stage-relations.json'
MAX_JSON=128*1024
PROFILE={'wordpress':'7.1.3','php':'8.5.12','initial_user':'null','relevant_filters':'empty',
         'hasher':'empty','locale':'explicit_php_truthy_bytes','charset':'UTF-8',
         'intl':False,'mbstring':False,'ctype_locale':'C','backend':'record_only_cache_miss_fake_sql_exact_key',
         'wp_user':'actual_class_reflection_without_constructor','rehash_write':'intent_only',
         'comparison':'ordered_stage_relation_not_real_authentication','maximum_raw_bytes':8192}
LIMITATIONS=[
 'Finite public dummies and fake record lookup, not DB, collation, cache policy or a real authentication result.',
 'No WordPress API or primitive comparison count is added; initial null and empty relevant filters only.',
 'No arbitrary prior WP_User/Error, application passwords, roles, user-ID policy, cookie/session or production login.',
 'Unsupported verification, host cost, lookup or hint outcomes stay terminal unresolved, never password mismatch.',
 'PHP refusal cases do not execute actual callbacks and are not claims of WordPress false/error results.',
 'Provider failure is covered by native injected tests only; no actual OS/provider fault is induced here.',
 'Rehash is only a predicate/intent; the fixture setter records intent and never writes a password.',
 'Output/input caps are postcapture protocol bounds, not RSS, allocator or wall-clock guarantees.',
 'Trusted stable local workspace observations, not signatures, continuous attestation or production credential handling.',
]

def result(outcome='error',tag=None,codes=(),lookups=(),hint=None,reason=None):
    return {'outcome':outcome,'record_tag':tag,'error_codes':list(codes),'lookups':list(lookups),
            'rehash_hint':hint,'unresolved_reason':reason}

def lookup(kind,key):return {'kind':kind,'key_hex':key.hex()}

def cases():
    rows=[]
    default=bcrypt.VECTORS['cost12'].encode()
    def rec(key,stored=default,tag='login-record'):
        return {'key_hex':key.hex(),'hash_hex':stored.hex(),'tag':tag}
    def add(name,raw=b'same@example.test',password=b'dummy-pass',login=None,email=None,expected=None,
            bcrypt_maximum=12,portable_maximum=13,lookup_error='none',actual=True,refusal=None,username_empty=None):
        codes=expected.pop('error_codes')
        expected['outcome']='no_match' if expected['outcome']=='error' else expected['outcome']
        expected['matched_via']=expected['lookups'][-1]['kind'] if expected['outcome']=='user' else None
        expected['unresolved_phase']=expected['lookups'][-1]['kind'] if expected['outcome']=='unresolved' else None
        expected['username_php_empty']=raw in (b'',b'0')if username_empty is None else username_empty
        expected['password_php_empty']=common.trim6(password)in(b'',b'0')
        rows.append({'id':name,'php_error_codes':codes,'request':{'username_hex':raw.hex(),'password_hex':password.hex(),
            'locale_hex':b'en_US'.hex(),'login':login,'email':email,'bcrypt_maximum':bcrypt_maximum,
            'portable_maximum':portable_maximum,'lookup_error':lookup_error},
            'expected':expected,'actual':actual,'refusal':refusal})
    key=b'same@example.test';l=lookup('login',key);e=lookup('email',key)
    add('login_correct_stops_email',login=rec(key),email=rec(key,tag='email-record'),
        expected=result('user','login-record',lookups=[l],hint=False))
    wrong=default.replace(b'$12$',b'$11$')
    add('wrong_login_then_email_success',login=rec(key,wrong),email=rec(key,tag='email-record'),
        expected=result('user','email-record',lookups=[l,e],hint=False))
    add('login_absent_then_email_success',email=rec(key,tag='email-record'),
        expected=result('user','email-record',lookups=[l,e],hint=False))
    add('invalid_email_preserves_username_error',raw=b'not-email',
        expected=result(codes=['invalid_username'],lookups=[lookup('login',b'not-email')]))
    add('valid_email_absent_replaces_username_error',
        expected=result(codes=['invalid_email'],lookups=[l,e]))
    add('both_wrong_email_error_wins',password=b'wrong',login=rec(key),email=rec(key,tag='email-record'),
        expected=result(codes=['incorrect_password'],lookups=[l,e]))
    for name,raw,password,codes in [('both_empty',b'',b'',['empty_username','empty_password']),
        ('both_zero',b'0',b'0',['empty_username','empty_password']),
        ('username_zero',b'0',b'dummy-pass',['empty_username']),
        ('password_zero',key,b'0',['empty_password']),('password_trim_empty',key,b' \t\0',['empty_password'])]:
        add(name,raw=raw,password=password,expected=result(codes=codes))
    add('ff_s1_space_lookup_skipped',raw=b'\f',expected=result(codes=['invalid_username']))
    add('s1_generated_zero_is_empty',raw=b'&x;0',username_empty=True,expected=result(codes=['empty_username']))
    add('s1_generated_empty_is_empty',raw=b'&x;',username_empty=True,expected=result(codes=['empty_username']))
    add('s2_empty_remains_query',raw=b'%&x;61',expected=result(codes=['invalid_username'],lookups=[lookup('login',b'')]))
    raw=b'%&x;61@example.test';s1=b'%61@example.test';s2=b'@example.test'
    add('s1_s2_distinct_login',raw=raw,login=rec(s2),expected=result('user','login-record',lookups=[lookup('login',s2)],hint=False))
    add('s1_s2_distinct_email',raw=raw,email=rec(s1,tag='email-record'),
        expected=result('user','email-record',lookups=[lookup('login',s2),lookup('email',s1)],hint=False))
    md5=hashlib.md5(b'dummy-pass',usedforsecurity=False).hexdigest().encode()
    add('md5_match_rehash_intent',login=rec(key,md5),expected=result('user','login-record',lookups=[l],hint=True))
    ppassword=legacy.VECTORS[1][1];portable=legacy.VECTORS[1][2].encode()
    add('portable_match_rehash_intent',password=ppassword,login=rec(key,portable),
        expected=result('user','login-record',lookups=[l],hint=True))
    add('bcrypt_cost4_match_rehash_intent',login=rec(key,bcrypt.VECTORS['dummy'].encode()),
        expected=result('user','login-record',lookups=[l],hint=True))
    add('trimmed_password_matches',password=b'\0 \tdummy-pass\r\n',login=rec(key),
        expected=result('user','login-record',lookups=[l],hint=False))
    binary=b'dummy\0\xffpass'
    add('binary_password_kept',password=binary,login=rec(key,bcrypt.VECTORS['binary'].encode()),
        expected=result('user','login-record',lookups=[l],hint=True))
    add('password4097_md5_before_guard',password=b'x'*4097,
        login=rec(key,hashlib.md5(b'x'*4097,usedforsecurity=False).hexdigest().encode()),
        expected=result('user','login-record',lookups=[l],hint=True))
    add('unknown_hash_long_password_definite_false',password=b'x'*4097,login=rec(key,b'?'*33),
        expected=result(codes=['invalid_email'],lookups=[l,e]))
    add('bcrypt_false_then_email_md5_match',password=b'x'*4097,login=rec(key),
        email=rec(key,hashlib.md5(b'x'*4097,usedforsecurity=False).hexdigest().encode(),'email-record'),
        expected=result('user','email-record',lookups=[l,e],hint=True))
    for name,stored,bmax,pmax,reason in [('unsupported',b'?'*33,12,13,'unsupported_hash'),
        ('bcrypt_cost',default,4,13,'bcrypt_cost_limit'),('portable_cost',portable,12,7,'portable_cost_limit')]:
        # portable vector here has cost7; substitute public cost13 for host refusal.
        if name=='portable_cost':stored=legacy.VECTORS[4][2].encode()
        add('login_unresolved_'+name,login=rec(key,stored),email=rec(key,tag='email-record'),
            bcrypt_maximum=bmax,portable_maximum=pmax,
            expected=result('unresolved',lookups=[l],reason=reason),actual=False,refusal=reason)
        add('email_unresolved_'+name,login=rec(key,b''),email=rec(key,stored,'email-record'),
            bcrypt_maximum=bmax,portable_maximum=pmax,
            expected=result('unresolved',lookups=[l,e],reason=reason),actual=False,refusal=reason)
    for stage in ('login','email'):
        trace=[l] if stage=='login' else [l,e]
        add(stage+'_lookup_unresolved',lookup_error=stage,
            expected=result('unresolved',lookups=trace,reason='lookup'),actual=False,refusal='lookup')
    return rows

def corpus_hash():return hashlib.sha256(common.canonical(cases())).hexdigest()

def expected_hooks(case):
    value=case['expected'];request=case['request']
    checks=sum(request[step['kind']]is not None and request[step['kind']]['key_hex']==step['key_hex']
               for step in value['lookups'])
    match=value['outcome']=='user'
    empty=value['username_php_empty']or value['password_php_empty']
    return {'sanitize_user':1+int(bool(value['lookups'])and value['lookups'][0]['kind']=='login'),
        'is_email':int(not empty and not(match and value['matched_via']=='login')),
        'wp_authenticate_user':checks,'check_password':checks,
        'wp_hash_password_algorithm':int(match),'wp_hash_password_options':int(match),'password_needs_rehash':int(match)}

def expected_php(case):
    actual=case['actual']
    return {'actual_called':actual,'username_calls':int(actual),'email_calls':int(actual),
        'relation':case['expected']if actual else None,'refusal':case['refusal'],
        'source_error_codes':case['php_error_codes']if actual else [],
        'preserved_username_error':case['id']in('invalid_email_preserves_username_error','both_empty','both_zero',
            'username_zero','password_zero','password_trim_empty','ff_s1_space_lookup_skipped','s2_empty_remains_query',
            's1_generated_zero_is_empty','s1_generated_empty_is_empty')if actual else None,
        'hook_counts':expected_hooks(case)if actual else None}

def tracked_files():
    return {*common.tracked_files(),'scripts/check_credential_stage_evidence.py','scripts/test_credential_stage_evidence.py',
        'scripts/compare_credential_stage.py','scripts/credential_stage_oracle.php','scripts/wordpress_reference.py',
        'scripts/check_legacy_password_evidence.py','scripts/legacy_password_oracle.php','scripts/wp_bcrypt_profile.py',
        *['.reference/wordpress-7.1.3/wp-includes/'+name for name in
          ('user.php','class-wp-user.php','class-wp-error.php','load.php','formatting.php','l10n.php','utf8.php','compat-utf8.php')]}

def binary_paths():
    return {'rust':common.binary_paths()['rust'].replace('password_generation_probe','credential_stage_probe'),
            'php':common.binary_paths()['php']}

def verify_pinned():
    common.verify_pinned()
    source=ROOT/'.reference/wordpress-7.1.3'
    with (source/'recastpress-reference.json').open('rb')as stream:wire=stream.read(8*1024*1024+1)
    if len(wire)>8*1024*1024:raise ValueError('reference manifest bound')
    manifest=json.loads(wire,object_pairs_hook=common.unique_object)
    hashes={row['path']:row['sha256']for row in manifest['files']}
    for path in tracked_files():
        prefix='.reference/wordpress-7.1.3/'
        if path.startswith(prefix):
            relative=path[len(prefix):]
            if relative!='recastpress-reference.json' and common.digest(ROOT/path)!=hashes[relative]:
                raise ValueError('pinned callback source differs')

def validate_record(record,verify_files=True):
    common.fields(record,['format_version','scope','generated_at_utc','profile','corpus_sha256','runs',
                          'oracle','cases','tracked_files','binaries','limitations'])
    if not common.exact(record['format_version'],1)or record['scope']!='credential_stage_public_dummy_relations':raise ValueError('stage scope differs')
    if not common.exact(record['profile'],PROFILE)or not common.exact(record['limitations'],LIMITATIONS)or record['corpus_sha256']!=corpus_hash():raise ValueError('stage profile/corpus differs')
    stamp=record['generated_at_utc']
    if type(stamp)is not str or 'T'not in stamp or datetime.fromisoformat(stamp).utcoffset()!=timedelta(0):raise ValueError('stage timestamp must be UTC')
    common.fields(record['runs'],['rust','php'])
    for run in record['runs'].values():
        common.fields(run,['owned_pid','reported_pid','exit_code','stderr_empty'])
        common.positive_pid(run['owned_pid']);common.positive_pid(run['reported_pid'])
        if not common.exact(run['owned_pid'],run['reported_pid'])or not common.exact(run['exit_code'],0)or run['stderr_empty']is not True:raise ValueError('stage child did not succeed')
    if not common.exact(record['oracle'],{'php_version':'8.5.12','globals_restored':True,'actions_restored':True,
        'restoration_states':['absent','null','sentinel','empty'],'db_writes':0}):raise ValueError('stage oracle context differs')
    expected=cases()
    if type(record['cases'])is not list or len(record['cases'])!=len(expected):raise ValueError('stage case coverage differs')
    for row,case in zip(record['cases'],expected,strict=True):
        common.fields(row,['id','native','php'])
        if row['id']!=case['id']or not common.exact(row['native'],case['expected']):raise ValueError('stage native relation differs')
        if not common.exact(row['php'],expected_php(case)):raise ValueError('stage actual/refusal relation differs')
    common.fields(record['tracked_files'],tracked_files())
    for path,value in record['tracked_files'].items():
        if type(value)is not str or not re.fullmatch('[0-9a-f]{64}',value):raise ValueError('invalid stage source hash')
        if verify_files and value!=common.digest(ROOT/path):raise ValueError('stale stage source: '+path)
    common.fields(record['binaries'],binary_paths())
    for name,path in binary_paths().items():
        item=record['binaries'][name];common.fields(item,['path','sha256'])
        if item['path']!=path or type(item['sha256'])is not str or not re.fullmatch('[0-9a-f]{64}',item['sha256']):raise ValueError('stage binary identity differs')
        if verify_files and item['sha256']!=common.digest(ROOT/path):raise ValueError('stale stage binary')
    return record

def main():
    verify_pinned();validate_record(common.read_json(OUTPUT))
    print('Credential stage relations match current sources/binaries; record-only public dummy fixture')

if __name__=='__main__':main()
