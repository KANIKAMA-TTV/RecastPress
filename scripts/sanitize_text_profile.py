"""No-literal-< fixed-UTF8-site text/textarea return values, empty filters."""
from collections import Counter
import random

APIS=('sanitize_text_field','sanitize_textarea_field')
NAME='raw_no_lt_fixed_utf8_no_mbstring_empty_filters'
LIMITS={'max_input_bytes':4096,'max_output_bytes':4096,'max_percent_rounds':1365}
OLD_COUNT=513292
OLD_INPUT_SHA256='271174a01e6316887321d59e2b158d63601a2b2863cf029170a29cf9b3383c38'
OLD_OUTPUT_SHA256='c1ba3118d76161dc14ae5a80467fa95471cc1be3d3a3dd73a6db687e35361cb4'
PROFILE={
 'php_version':'8.5.12','name':NAME,'input_type':'arbitrary_bytes_without_literal_lt',
 'blog_charset':'UTF-8','option_fixture':'immutable_utf8_not_core_option_resolution',
 'mbstring_loaded':False,'mb_check_encoding_exists':False,'mb_convert_encoding_exists':False,
 'ctype_locale':'C','invalid_utf8':'whole_input_empty_before_percent_processing',
 'utf8_static':'reflection_null_or_true_before_empty_keeps_state_nonempty_after_true_no_prime',
 'utf8_internal_calls':'uninstrumented_actual_source_chain',
 'html_branch':'source_unreachable_raw_no_lt_and_strip_false_not_measured_calls',
 'filter_registry':'one_empty_own_wp_hook_no_all_callbacks','filter_arguments':2,
 'filter_arguments_basis':'pinned_source_without_observer_callback','native_filter_bookkeeping':False,
 'percent_order':'first_case_insensitive_match_exact_selected_three_byte_global_nonrecursive_delete_repeat',
 'percent_rounds':'successful_selected_triplet_deletion_passes_only',
 'round_budget_basis':'raw_input_4096_bytes_floor_div_three_is_sufficient',
 'fixture_limits':LIMITS,
 'source_sha256':'350f6b32303915df970bb0e3ea360fcd54bb0b8a7ecc3d504c3d860ab9468d32',
}
COUNT_KEYS=('calls','field_hook','area_hook','restored')


def exact(value,expected):
 if type(value)is not type(expected):return False
 if type(expected)is dict:return set(value)==set(expected)and all(exact(value[k],v)for k,v in expected.items())
 return value==expected


def validate_profile(value):
 if not exact(value,PROFILE):raise ValueError('Text sanitizer profile differs')


def validate_counts(value,expected):
 for item in(value,expected):
  if type(item)is not dict or set(item)!=set(COUNT_KEYS)or any(type(item[k])is not int or item[k]<0 for k in COUNT_KEYS):
   raise ValueError('Text sanitizer counts differ')
 if value!=expected:raise ValueError('Text sanitizer counts differ')


def validate_previous(count,input_sha,rust_sha,php_sha):
 if(type(count)is not int or count!=OLD_COUNT or type(input_sha)is not str or input_sha!=OLD_INPUT_SHA256
    or type(rust_sha)is not str or rust_sha!=OLD_OUTPUT_SHA256 or type(php_sha)is not str or php_sha!=OLD_OUTPUT_SHA256):
  raise ValueError('Previous API77 exact corpus/results changed')


def fixture(raw=b'public text',api='sanitize_text_field'):
 return{'api':api,'input':{'type':'string','hex':raw.hex()},'text_profile':NAME,
        'empty_filters':True,'limits':dict(LIMITS)}


def expected_counts(cases):
 selected=[case for case in cases if case['api']in APIS]
 return{'calls':len(selected),'field_hook':sum(case['api']==APIS[0]for case in selected),
        'area_hook':sum(case['api']==APIS[1]for case in selected),'restored':len(selected)}


def aggregate_counts(batches):
 total=Counter(dict.fromkeys(COUNT_KEYS,0))
 for batch in batches:total.update(expected_counts(batch))
 return dict(total)


def cases():
 raws=[b'',b'0',b' ',b'\0',b'\x0b',b'\x0c',b' a\t\r\nb ',b'a  b',b'a  b %41',b'a  b %GG',
       b'a\t%41\tb',b'\0%41\0A\0',b'a\0b',b'&lt;',b'%3cscript%3e',b'%c3%a9',
       b'%a%41b',b'%4%411',b'%aB%ab%aB',b'%a%41b%AB',b'%%41',b'%41%41',b'%41%4',
       b'\xc3%41\xa9',b'\xc3\0\xa9',b'\xff%41',b'\xed\xa0\x80',b'\xf4\x90\x80\x80',
       '\u00a0 A \u00a0'.encode(),'日本語\u2003é🦀'.encode(),b'\x0cA\x0c']
 for byte in range(256):
  raw=bytes([byte])
  if raw!=b'<':raws.extend([raw,b'A'+raw+b'B',b'A'+raw+b'%41'+raw+b'B'])
  lower=f'%{byte:02x}'.encode();upper=lower.upper()
  raws.extend([lower,upper,lower+upper+lower,b'A '+lower+b'  B',b'%a'+lower+b'b'])
 for depth in (1,2,3,8,32,128,1364):
  raw=b'%a'*depth+b'%41'+b'b'*depth
  raws.extend([raw,b'x'+raw])
 generator=random.Random(20261030078)
 palette=[b'A',b'0',b' ',b'\t',b'\n',b'\r',b'\x0b',b'\x0c',b'\0',b'%',b'%a',b'%41',b'%aB',b'%ab',b'%GG',b'&lt;',b'\xff','é'.encode(),'日本'.encode()]
 for _ in range(768):raws.append(b''.join(generator.choice(palette)for _ in range(generator.randrange(48))))
 raws.extend([b'x'*4096,b' '*4096,b'%41'*1365+b'x'])
 result=[]
 for raw in raws:
  if len(raw)>4096 or b'<'in raw:raise ValueError('Text sanitizer corpus escaped profile')
  for api in APIS:result.append(fixture(raw,api))
 return result
