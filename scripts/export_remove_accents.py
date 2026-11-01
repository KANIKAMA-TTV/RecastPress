"""Data-only extraction of pinned WP accent literals; no PHP execution or normalization."""
import argparse
import hashlib
import json
import re

import wordpress_reference as reference

SOURCE_PATH = 'wp-includes/formatting.php'
SOURCE_SHA256 = '350f6b32303915df970bb0e3ea360fcd54bb0b8a7ecc3d504c3d860ab9468d32'
BASE_SHA256 = '69b78d9596b1e927213e72b0698a40a85daa76e8846a4d86d7a13c78d4594477'
OVERRIDES_SHA256 = '07c40351a07697a814d6a97ec45c2549b7f08071f3ff3cc33768fa0285e6932c'
ARTIFACT = 'compatibility/wordpress-remove-accents.json'
RUST_DATA = 'crates/recastpress-wp/src/remove_accents_table.rs'
COUNTS = {'BASE':311, 'GERMAN':8, 'DANISH':6, 'CATALAN':1, 'SERBIAN_BOSNIAN':2}
GUARDS = ["if ( str_starts_with( $locale, 'de' ) ) {", "} elseif ( 'da_DK' === $locale ) {",
          "} elseif ( 'ca' === $locale ) {", "} elseif ( 'sr_RS' === $locale || 'bs_BA' === $locale ) {"]

def canonical(value):
    return json.dumps(value, ensure_ascii=True, separators=(',', ':')).encode('ascii')

def pairs_hash(pairs):
    return hashlib.sha256(canonical(pairs)).hexdigest()

def validate_pairs(pairs, base=False):
    seen=set()
    for key, value in pairs:
        raw=bytes.fromhex(key); out=bytes.fromhex(value)
        text=raw.decode('utf-8')
        if (not raw or len(raw)>4 or key in seen or (base and (len(text)!=1 or len(raw) not in (2,3)))
            or len(out)>2 or any(byte not in b'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz' for byte in out)
            or len(out)>len(raw)):
            raise ValueError('accent literal is duplicate or outside fixed byte grammar')
        seen.add(key)

def extract_tables(source):
    signature="function remove_accents( $text, $locale = '' ) {"
    if source.count(signature)!=1: raise ValueError('accent function missing or ambiguous')
    function=source.split(signature,1)[1].split('\nfunction sanitize_file_name(',1)[0]
    if len(re.findall(r'\$chars\s*=\s*array\((?!\s*\))',function))!=1:
        raise ValueError('base literal assignment ambiguous')
    opening='$chars = array('
    start=function.find(opening)
    if start<0: raise ValueError('base literal missing')
    end=function.find('\n\t\t);',start)
    if end<0: raise ValueError('base literal unterminated')
    pairs=[]
    for line in function[start+len(opening):end].splitlines():
        line=line.strip()
        if not line or line.startswith('//'): continue
        match=re.fullmatch(r"'([^'\\]+)'\s*=>\s*'([A-Za-z]*)',",line)
        if not match: raise ValueError('base literal contains executable or ambiguous data')
        pairs.append([match[1].encode('utf-8').hex(),match[2].encode('ascii').hex()])
    validate_pairs(pairs,base=True)
    tables={'BASE':pairs}
    after=function[end+len('\n\t\t);'):]
    if after.count(GUARDS[0])!=1: raise ValueError('locale dispatch missing or ambiguous')
    block=after.split(GUARDS[0],1)[1]
    terminator='$text = strtr( $text, $chars );'
    if block.count(terminator)!=1: raise ValueError('locale translation terminal differs')
    block=block.split(terminator,1)[0]
    names=list(COUNTS)[1:]; current=0; tables[names[0]]=[]; closed=False
    for line in block.splitlines():
        line=line.strip()
        if not line: continue
        if current+1<len(GUARDS) and line==GUARDS[current+1]:
            current+=1; tables[names[current]]=[];continue
        if line=='}':
            if current!=len(names)-1 or closed: raise ValueError('locale branch closing differs')
            closed=True;continue
        match=re.fullmatch(r"\$chars\['([^'\\]+)'\]\s*=\s*'([A-Za-z]*)';",line)
        if not match or closed: raise ValueError('locale literal or guard differs')
        tables[names[current]].append([match[1].encode('utf-8').hex(),match[2].encode('ascii').hex()])
    if not closed or list(tables)!=list(COUNTS): raise ValueError('locale branch set differs')
    for name,pairs in tables.items(): validate_pairs(pairs,base=name=='BASE')
    return tables

def profile():
    return {'source_sha256':SOURCE_SHA256,'base_count':311,'base_ordered_pairs_sha256':BASE_SHA256,
            'locale_override_ordered_pairs_sha256':OVERRIDES_SHA256}

def generated():
    path=reference.REFERENCE / f'wordpress-{reference.VERSION}' / SOURCE_PATH
    with path.open('rb') as stream: raw=stream.read(1024*1024+1)
    if len(raw)>1024*1024 or hashlib.sha256(raw).hexdigest()!=SOURCE_SHA256:
        raise ValueError('accent source identity differs')
    tables=extract_tables(raw.decode('utf-8'))
    overrides=[[name,tables[name]] for name in list(COUNTS)[1:]]
    if ({name:len(pairs)for name,pairs in tables.items()}!=COUNTS or pairs_hash(tables['BASE'])!=BASE_SHA256
        or pairs_hash(overrides)!=OVERRIDES_SHA256): raise ValueError('accent table count/order differs')
    record={'format_version':1,'wordpress_version':reference.VERSION,'source_path':SOURCE_PATH,'source_line':1611,
            'base_literal_line':1630,**profile(),'counts':COUNTS,'tables':tables}
    rust='// Generated from WordPress 7.1.3 formatting.php:1611 by scripts/export_remove_accents.py.\n'
    rust+='// Source-derived GPL-2.0-or-later data; byte pairs only, no locale/settings policy.\n'
    for name,pairs in tables.items():
        entries=[]
        for key,value in sorted(pairs,key=lambda pair:bytes.fromhex(pair[0])):
            escaped=''.join(f'\\x{byte:02x}'for byte in bytes.fromhex(key))
            entries.append(f'(b"{escaped}", b"{bytes.fromhex(value).decode("ascii")}")')
        prefix=f'pub(crate) const {name}: &[(&[u8], &[u8])] = &['
        if len(entries)<=2:
            rust+=prefix+', '.join(entries)+'];\n'
        else:
            rust+=prefix+'\n'+''.join('    '+entry+',\n'for entry in entries)+'];\n'
    return json.dumps(record,ensure_ascii=True,indent=2)+'\n',rust

def check():
    for path,wire in zip((ARTIFACT,RUST_DATA),generated(),strict=True):
        expected=wire.encode('utf-8')
        with (reference.ROOT/path).open('rb') as stream: actual=stream.read(len(expected)+1)
        if actual!=expected: raise ValueError('accent generated data is stale: '+path)

def read_tables():
    with (reference.ROOT/ARTIFACT).open('rb') as stream: wire=stream.read(131073)
    if len(wire)>131072: raise ValueError('accent artifact read bound')
    tables=json.loads(wire)['tables']
    if not isinstance(tables,dict) or set(tables)!=set(COUNTS): raise ValueError('accent table set differs')
    for name,pairs in tables.items():
        if (type(pairs) is not list or len(pairs)!=COUNTS[name]
            or any(type(pair) is not list or len(pair)!=2 or any(type(value) is not str or not re.fullmatch(r'(?:[0-9a-f]{2})*',value)for value in pair)for pair in pairs)):
            raise ValueError('accent table record grammar differs')
        validate_pairs(pairs,base=name=='BASE')
    if (pairs_hash(tables['BASE'])!=BASE_SHA256 or pairs_hash([[name,tables[name]]for name in list(COUNTS)[1:]])!=OVERRIDES_SHA256):
        raise ValueError('recorded accent table identity differs')
    return tables

def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--check',action='store_true');args=parser.parse_args()
    if args.check: check()
    else:
        for path,wire in zip((ARTIFACT,RUST_DATA),generated(),strict=True):
            (reference.ROOT/path).write_text(wire,encoding='utf-8',newline='\n')
    print('Pinned accent data matches: base311 and explicit locale overrides; no PHP execution')

if __name__=='__main__': main()
