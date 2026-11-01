"""Meaningful relation/refusal/provenance contracts; synthetic records are not proof."""
import copy
from datetime import datetime,timezone
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import check_credential_stage_evidence as checker
import compare_credential_stage as comparison
from owned_probe import run_captured_probe,reap_owned_probes

def valid_record():
    return {'format_version':1,'scope':'credential_stage_public_dummy_relations',
        'generated_at_utc':datetime.now(timezone.utc).isoformat(),'profile':copy.deepcopy(checker.PROFILE),
        'corpus_sha256':checker.corpus_hash(),'runs':{key:{'owned_pid':1234,'reported_pid':1234,'exit_code':0,'stderr_empty':True}for key in ('rust','php')},
        'oracle':{'php_version':'8.5.12','globals_restored':True,'actions_restored':True,
                  'restoration_states':['absent','null','sentinel','empty'],'db_writes':0},
        'cases':[{'id':case['id'],'native':case['expected'],'php':checker.expected_php(case)}for case in checker.cases()],
        'tracked_files':dict.fromkeys(checker.tracked_files(),'a'*64),
        'binaries':{name:{'path':path,'sha256':'b'*64}for name,path in checker.binary_paths().items()},
        'limitations':checker.LIMITATIONS.copy()}

class CredentialStageEvidenceTests(unittest.TestCase):
    def rejected(self,mutate):
        record=valid_record();mutate(record)
        with self.assertRaises((ValueError,TypeError)):checker.validate_record(record,verify_files=False)

    def test_unresolved_is_terminal_and_never_wp_false_or_cross_identity_success(self):
        checker.validate_record(valid_record(),verify_files=False)
        for i,case in enumerate(checker.cases()):
            if case['expected']['outcome']=='unresolved':
                self.rejected(lambda value,i=i:value['cases'][i]['native'].__setitem__('outcome','no_match'))
                self.rejected(lambda value,i=i:value['cases'][i]['php'].__setitem__('actual_called',True))
                self.rejected(lambda value,i=i:value['cases'][i]['php'].__setitem__('username_calls',1))
                self.rejected(lambda value,i=i:value['cases'][i]['native'].__setitem__('unresolved_phase','input'))
        rows={r['id']:r for r in checker.cases()}
        self.assertEqual(rows['login_unresolved_unsupported']['expected']['lookups'],[checker.lookup('login',b'same@example.test')])
        self.assertEqual(rows['email_unresolved_bcrypt_cost']['expected']['unresolved_phase'],'email')

    def test_s1_s2_derived_php_empty_and_final_match_are_distinct_relations(self):
        rows={r['id']:r for r in checker.cases()}
        self.assertTrue(rows['s1_generated_zero_is_empty']['expected']['username_php_empty'])
        self.assertTrue(rows['s1_generated_empty_is_empty']['expected']['username_php_empty'])
        self.assertFalse(rows['ff_s1_space_lookup_skipped']['expected']['username_php_empty'])
        self.assertEqual(rows['s2_empty_remains_query']['expected']['lookups'],[checker.lookup('login',b'')])
        self.assertEqual(rows['s1_s2_distinct_email']['expected']['lookups'],[checker.lookup('login',b'@example.test'),checker.lookup('email',b'%61@example.test')])
        self.rejected(lambda value:value['cases'][0]['native'].__setitem__('matched_via','email'))
        self.rejected(lambda value:value['cases'][0]['native'].__setitem__('record_tag','email-record'))

    def test_source_error_identity_hook_counts_and_hint_are_independent_of_native_no_match(self):
        rows={r['id']:i for i,r in enumerate(checker.cases())}
        i=rows['invalid_email_preserves_username_error']
        self.rejected(lambda value:value['cases'][i]['php'].__setitem__('preserved_username_error',False))
        self.rejected(lambda value:value['cases'][i]['php'].__setitem__('source_error_codes',['invalid_email']))
        i=rows['md5_match_rehash_intent']
        self.rejected(lambda value:value['cases'][i]['native'].__setitem__('rehash_hint',False))
        self.rejected(lambda value:value['cases'][i]['php']['hook_counts'].__setitem__('password_needs_rehash',True))
        i=rows['login_correct_stops_email']
        self.rejected(lambda value:value['cases'][i]['php']['hook_counts'].__setitem__('is_email',1))

    def test_strict_types_exact_fields_complete_coverage_and_restored_context(self):
        for mutate in (lambda v:v.__setitem__('format_version',True),lambda v:v['profile'].__setitem__('intl',0),
                       lambda v:v['cases'].pop(),lambda v:v['cases'].reverse(),
                       lambda v:v['cases'][0]['native'].__setitem__('extra',True),
                       lambda v:v['oracle'].__setitem__('globals_restored',False),
                       lambda v:v['oracle'].__setitem__('actions_restored',False),
                       lambda v:v['oracle'].__setitem__('db_writes',False),
                       lambda v:v['oracle']['restoration_states'].pop()):self.rejected(mutate)

    def test_current_pids_pinned_sources_binary_identity_and_utc_are_required(self):
        for mutate in (lambda v:v['runs']['rust'].__setitem__('owned_pid',True),
                       lambda v:v['runs']['php'].__setitem__('reported_pid',999),
                       lambda v:v['runs']['php'].__setitem__('stderr_empty',False),
                       lambda v:v['runs']['rust'].__setitem__('exit_code',False),
                       lambda v:v['tracked_files'].pop('.reference/wordpress-7.1.3/wp-includes/class-wp-user.php'),
                       lambda v:v['binaries']['rust'].__setitem__('path','foreign.exe'),
                       lambda v:v.__setitem__('generated_at_utc','2026-10-29T00:00:00')):self.rejected(mutate)
        with self.assertRaises(ValueError):checker.validate_record(valid_record())

    def test_bounded_duplicate_json_and_outer_owned_cleanup_preserve_original_failure(self):
        with tempfile.TemporaryDirectory(dir=checker.ROOT/'target')as temporary:
            path=Path(temporary)/'stage.json'
            for wire in (b'{"scope":"x","scope":"y"}',b'\xff',b' '*(checker.MAX_JSON+1)):
                path.write_bytes(wire)
                with self.assertRaises((ValueError,UnicodeError)):checker.common.read_json(path)
        original=ValueError('fixed synthetic failure')
        with patch.object(comparison,'compare',side_effect=original),patch.object(comparison,'reap_owned_probes',side_effect=RuntimeError('cleanup'))as cleanup:
            with self.assertRaises(ValueError)as raised:comparison.main()
            self.assertIs(raised.exception,original);cleanup.assert_called_once()
            self.assertEqual(original.__notes__,['Owned credential stage cleanup retry failed: RuntimeError'])

    @unittest.skipUnless((checker.ROOT/checker.binary_paths()['php']).is_file(),'pinned development PHP unavailable')
    def test_actual_oracle_rejects_bad_protocol_without_stdout_or_secret_trace(self):
        base=checker.cases()[0]['request']
        bad=[]
        for field,value in [('username_hex',None),('password_hex','SECRET_STAGE_INPUT'),('locale_hex',''),
                            ('bcrypt_maximum',True),('portable_maximum',13.0),('lookup_error','bad')]:
            item=copy.deepcopy(base);item[field]=value;bad.append({'items':[item]})
        item=copy.deepcopy(base);item['login']['hash_hex']=False;bad.append({'items':[item]})
        item=copy.deepcopy(base);item['username_hex']=b'SECRET_STAGE_INPUT<'.hex();bad.append({'items':[item]})
        wires=[checker.common.canonical(x)for x in bad]+[b'{"items":[],"items":[]}']
        command=[str(checker.ROOT/checker.binary_paths()['php']),'-n','-d','memory_limit=128M',str(checker.ROOT/'scripts/credential_stage_oracle.php'),str(checker.ROOT/'.reference/wordpress-7.1.3')]
        try:
            for wire in wires:
                _,result=run_captured_probe(command,os.environ.copy(),timeout=15,input_bytes=wire)
                self.assertNotEqual(result.returncode,0);self.assertEqual(result.stdout,b'')
                self.assertNotIn(b'SECRET_STAGE_INPUT',result.stderr)
        finally:reap_owned_probes()

    @unittest.skipUnless((checker.ROOT/checker.binary_paths()['rust']).is_file(),'current development probe unavailable')
    def test_native_probe_refuses_bad_types_duplicates_and_secret_inputs_without_result(self):
        command=[str(checker.ROOT/checker.binary_paths()['rust'])]
        base=checker.cases()[0]['request'];wires=[b'{"items":[],"items":[]}']
        for field,value in [('username_hex','SECRET_STAGE_NATIVE'),('locale_hex',''),
                            ('bcrypt_maximum',True),('portable_maximum',13.0),('lookup_error',None)]:
            item=copy.deepcopy(base);item[field]=value;wires.append(checker.common.canonical({'items':[item]}))
        item=copy.deepcopy(base);item['login']['tag']='SECRET_STAGE_NATIVE';wires.append(checker.common.canonical({'items':[item]}))
        try:
            for wire in wires:
                _,result=run_captured_probe(command,os.environ.copy(),timeout=15,input_bytes=wire)
                self.assertNotEqual(result.returncode,0);self.assertEqual(result.stdout,b'')
                self.assertEqual(result.stderr,b'Credential stage probe rejected\n')
        finally:reap_owned_probes()

if __name__=='__main__':unittest.main()
