"""Data-only parser and synthetic wrapper contracts, separate from actual WP comparisons."""
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

import export_remove_accents as data

ROOT=Path(__file__).resolve().parents[1]
PHP=ROOT/'.reference/php-8.5.12/php.exe'

def fixture():
    result="function remove_accents( $text, $locale = '' ) {\n$chars = array(\n'ª' => 'a',\n'£' => '',\n\t\t);\n"
    for guard, (key,value) in zip(data.GUARDS,[('Ä','Ae'),('Å','Aa'),('l·l','ll'),('Đ','DJ')],strict=True):
        result+=guard+"\n$chars['"+key+"'] = '"+value+"';\n"
    return result+"}\n$text = strtr( $text, $chars );\n}\nfunction sanitize_file_name("

class AccentDataTests(unittest.TestCase):
    def test_data_only_literal_parser_retains_empty_values_and_exact_locale_guards(self):
        tables=data.extract_tables(fixture())
        self.assertEqual(tables['BASE'],[['c2aa','61'],['c2a3','']])
        self.assertEqual(tables['CATALAN'],[['6cc2b76c','6c6c']])
        self.assertEqual(list(tables),list(data.COUNTS))
        for changed in [fixture().replace("'ª' => 'a',","'ª' => 'a',\n'ª' => 'a',"),
            fixture().replace("'ª' => 'a',","'ª' => execute(),"),
            fixture().replace("'ª' => 'a',","'A' => 'a',"),
            fixture().replace("'ª' => 'a',","'ª' => 'aaa',"),
            fixture().replace("'de' )","'de_' )"),
            fixture().replace("$chars['l·l']","$chars['l·lx']"),
            fixture()+fixture()]:
            with self.assertRaises(ValueError): data.extract_tables(changed)

    @unittest.skipUnless((data.reference.REFERENCE/'wordpress-7.1.3'/data.SOURCE_PATH).is_file(),'pinned source unavailable')
    def test_generated_artifact_and_rust_bytes_are_reproducible_and_both_required(self):
        artifact,rust=data.generated()
        record=json.loads(artifact)
        self.assertEqual(record['counts'],data.COUNTS)
        self.assertEqual(data.pairs_hash(record['tables']['BASE']),data.BASE_SHA256)
        self.assertIn('pub(crate) const CATALAN:',rust)
        with tempfile.TemporaryDirectory(dir=ROOT/'target') as temporary:
            root=Path(temporary)
            for path,wire in zip((data.ARTIFACT,data.RUST_DATA),(artifact,rust),strict=True):
                (root/path).parent.mkdir(parents=True,exist_ok=True);(root/path).write_text(wire,encoding='utf-8',newline='\n')
            with patch.object(data.reference,'ROOT',root):
                data.check();data.read_tables()
                for path in (data.ARTIFACT,data.RUST_DATA):
                    original=(root/path).read_bytes();(root/path).write_bytes(original+b' ')
                    with self.assertRaises(ValueError):data.check()
                    (root/path).write_bytes(original)

@unittest.skipUnless(PHP.is_file(),'pinned development PHP unavailable')
class AccentOracleTests(unittest.TestCase):
    def php(self,code,*args,success=True):
        result=subprocess.run([str(PHP),'-n','-r',code,str(ROOT),*args],stdout=subprocess.PIPE,stderr=subprocess.PIPE,timeout=15)
        if success:
            self.assertEqual(result.returncode,0,result.stderr);self.assertEqual(result.stderr,b'')
            return json.loads(result.stdout)
        self.assertNotEqual(result.returncode,0);self.assertEqual(result.stdout,b'')
        return result

    def test_explicit_raw_locale_forwarding_and_bad_domain_do_not_call_locale_lookup(self):
        code=r'''
require $argv[1].'/scripts/reference_functions.php';
load_reference_function($argv[1].'/scripts/wp_oracle.php','oracle_remove_accents');
$remove_accents_calls=0;$remove_accents_get_locale_calls=0;$called=0;$locale_expected='';
function wp_is_valid_utf8($value){return preg_match('//u',$value)===1;}
function did_filter($name){return 0;}
function get_locale(){throw new LogicException('ambient locale read');}
function remove_accents($value,$locale){global$called,$locale_expected;if($locale!==$locale_expected){throw new LogicException('locale changed');}++$called;return$value;}
foreach(['en_US','de',"de\0\xff","\xff","\0",'00',"0\0"]as$locale){$locale_expected=$locale;
 foreach(['',"A\0",'Cafe' . "\xcc\x81"]as$value){if(oracle_remove_accents($value,$locale,'valid_utf8_explicit_locale_no_intl')!==$value){throw new LogicException('bytes changed');}}
}
$before=$called;
foreach([[null,'en_US'],[false,'en_US'],[[], 'en_US'],["\xff",'en_US'],['',''],['','0'],['',null],['',false],['',7]]as[$value,$locale]){
 $refused=false;try{oracle_remove_accents($value,$locale,'valid_utf8_explicit_locale_no_intl');}catch(RuntimeException$e){$refused=true;}
 if(!$refused||$before!==$called){throw new LogicException('bad input called reference');}
}
echo json_encode(['calls'=>$remove_accents_calls,'inner_calls'=>$called,'ambient'=>$remove_accents_get_locale_calls]),"\n";
'''
        self.assertEqual(self.php(code),{'calls':21,'inner_calls':21,'ambient':0})

    def test_each_normalizer_function_presence_and_inner_nonbyte_failure_are_closed(self):
        for definitions in ['function normalizer_is_normalized($v){return true;}',
                            'function normalizer_normalize($v){return $v;}',
                            'function normalizer_is_normalized($v){return true;}function normalizer_normalize($v){return $v;}']:
            code=r'''
require $argv[1].'/scripts/reference_functions.php';
load_reference_function($argv[1].'/scripts/wp_oracle.php','oracle_remove_accents');
$remove_accents_calls=0;$remove_accents_get_locale_calls=0;$called=0;
function wp_is_valid_utf8($v){return true;}function did_filter($name){return 0;}function remove_accents($v,$l){global$called;++$called;return$v;}
'''+definitions+r'''
$refused=false;try{oracle_remove_accents('test','en_US','valid_utf8_explicit_locale_no_intl');}catch(RuntimeException$e){$refused=true;}
echo json_encode(['refused'=>$refused,'calls'=>$called,'success'=>$remove_accents_calls]),"\n";
'''
            self.assertEqual(self.php(code),{'refused':True,'calls':0,'success':0})
        code=r'''
require $argv[1].'/scripts/reference_functions.php';
load_reference_function($argv[1].'/scripts/wp_oracle.php','oracle_remove_accents');
$remove_accents_calls=0;$remove_accents_get_locale_calls=0;
function wp_is_valid_utf8($v){return true;}function did_filter($name){return 0;}function remove_accents($v,$l){return false;}
$refused=false;try{oracle_remove_accents('test','en_US','valid_utf8_explicit_locale_no_intl');}catch(RuntimeException$e){$refused=true;}
echo json_encode(['refused'=>$refused,'success'=>$remove_accents_calls]),"\n";
'''
        self.assertEqual(self.php(code),{'refused':True,'success':0})

    def test_exception_trace_redacts_text_locale_and_profile(self):
        code=r'''
ini_set('display_errors','stderr');ini_set('log_errors','0');ini_set('zend.exception_ignore_args','0');
require $argv[1].'/scripts/reference_functions.php';
load_reference_function($argv[1].'/scripts/wp_oracle.php','oracle_remove_accents');
oracle_remove_accents('SECRET_ACCENT_TEXT','SECRET_ACCENT_LOCALE','SECRET_ACCENT_PROFILE');
'''
        result=self.php(code,success=False)
        self.assertNotIn(b'SECRET_ACCENT',result.stderr)

if __name__=='__main__':unittest.main()
