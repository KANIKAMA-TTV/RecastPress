"""Test-only loopback CA/leaf generation with an existing local OpenSSL.

The caller supplies an existing empty owned directory under this checkout's
target/. No existing DB CA/key is read, no tools are installed, and no generated
key or tool output is printed. This is not a production CA/salt/key policy.
Ordinary path checks assume a trusted stable workspace, not hostile-race defense.
"""
import os
from pathlib import Path
import shutil
import stat
import sys

from owned_probe import run_captured_probe, reap_owned_probes

ROOT=Path(__file__).resolve().parents[1]


class LoopbackTlsFixtureError(RuntimeError):
    """Closed diagnostic with no child output, private key or arbitrary path."""


def available_openssl():
    """Only existing Git-for-Windows OpenSSL or Linux PATH; never install/fetch."""
    if os.name=='nt':
        candidate=Path('C:/Program Files/Git/usr/bin/openssl.exe')
        return candidate if candidate.is_file()else None
    if sys.platform=='linux':
        value=shutil.which('openssl')
        return Path(value).resolve()if value else None
    return None


def _owned_empty(directory):
    directory=Path(directory)
    root=ROOT.resolve(strict=True);target=root/'target'
    resolved=directory.resolve(strict=True)
    if resolved==target or not resolved.is_relative_to(target)or not resolved.is_dir():
        raise LoopbackTlsFixtureError('loopback TLS fixture directory refused')
    # Reject detected symlinks/reparse points in the caller-controlled suffix.
    # Resolution/containment does not eliminate a concurrent directory swap.
    lexical=directory.absolute()
    if not lexical.is_relative_to(target):
        raise LoopbackTlsFixtureError('loopback TLS fixture directory refused')
    for path in (target,*[target.joinpath(*lexical.relative_to(target).parts[:i])for i in range(1,len(lexical.relative_to(target).parts)+1)]):
        metadata=path.lstat()
        if stat.S_ISLNK(metadata.st_mode)or getattr(metadata,'st_file_attributes',0)&getattr(stat,'FILE_ATTRIBUTE_REPARSE_POINT',0x400):
            raise LoopbackTlsFixtureError('loopback TLS fixture directory refused')
    if any(resolved.iterdir()):
        raise LoopbackTlsFixtureError('loopback TLS fixture directory must be empty')
    return resolved


CA_CONFIG='''[req]
prompt = no
distinguished_name = dn
x509_extensions = ca_ext
[dn]
CN = RecastPress public loopback test CA
[ca_ext]
basicConstraints = critical,CA:TRUE
keyUsage = critical,keyCertSign,cRLSign
subjectKeyIdentifier = hash
authorityKeyIdentifier = keyid:always,issuer
'''

LEAF_CONFIG='''[req]
prompt = no
distinguished_name = dn
req_extensions = request_ext
[dn]
CN = localhost
[request_ext]
subjectAltName = DNS:localhost
[server_ext]
basicConstraints = critical,CA:FALSE
keyUsage = critical,digitalSignature,keyEncipherment
extendedKeyUsage = serverAuth
subjectAltName = DNS:localhost
subjectKeyIdentifier = hash
authorityKeyIdentifier = keyid:always,issuer
'''


def generate_loopback_tls(directory:Path)->tuple[Path,Path,Path]:
    """Return (CA certificate, localhost-only leaf certificate, leaf key).

    RSA2048/SHA256/day1 fixtures only. Partial files are kept in this same owned
    directory on failure for caller cleanup; no existing artifacts are replaced.
    Captured subprocess waits/cleanup are bounded; outputs are checked after
    capture and are not claimed as an allocator/RSS bound or secret erasure.
    """
    original=None
    try:
        tool=available_openssl()
        if tool is None:raise LoopbackTlsFixtureError('existing OpenSSL is unavailable')
        directory=_owned_empty(directory)
        ca_config=directory/'ca-config.cnf';leaf_config=directory/'leaf-config.cnf'
        with ca_config.open('x',encoding='ascii',newline='\n')as stream:stream.write(CA_CONFIG)
        with leaf_config.open('x',encoding='ascii',newline='\n')as stream:stream.write(LEAF_CONFIG)
        ca_cert=directory/'ca-cert.pem';ca_key=directory/'ca-key.pem'
        cert=directory/'server-cert.pem';key=directory/'server-key.pem';csr=directory/'server.csr'
        environment={name:value for name,value in os.environ.items()if name.upper()not in ('SSLKEYLOGFILE','RANDFILE')}
        environment['MSYS_NO_PATHCONV']='1'
        environment['OPENSSL_CONF']=str(ca_config)
        commands=[
            ['req','-new','-x509','-newkey','rsa:2048','-nodes','-sha256','-days','1','-set_serial','1',
             '-config',str(ca_config),'-keyout',str(ca_key),'-out',str(ca_cert)],
            ['req','-new','-newkey','rsa:2048','-nodes','-sha256','-config',str(leaf_config),
             '-keyout',str(key),'-out',str(csr)],
            ['x509','-req','-in',str(csr),'-CA',str(ca_cert),'-CAkey',str(ca_key),'-set_serial','2',
             '-days','1','-sha256','-extfile',str(leaf_config),'-extensions','server_ext','-out',str(cert)],
            ['verify','-x509_strict','-purpose','sslserver','-verify_hostname','localhost','-CAfile',str(ca_cert),str(cert)],
        ]
        for arguments in commands:
            _,result=run_captured_probe([str(tool),*arguments],environment,timeout=20)
            if result.returncode!=0 or len(result.stdout)>65536 or len(result.stderr)>65536:
                raise LoopbackTlsFixtureError('loopback TLS fixture generation failed')
        for path in (ca_cert,ca_key,cert,key,csr):
            info=path.lstat()
            if not stat.S_ISREG(info.st_mode)or info.st_size<=0 or info.st_size>65536 or getattr(info,'st_file_attributes',0)&getattr(stat,'FILE_ATTRIBUTE_REPARSE_POINT',0x400):
                raise LoopbackTlsFixtureError('loopback TLS fixture output refused')
        reap_owned_probes()
        return ca_cert,cert,key
    except BaseException as error:
        if isinstance(error,(KeyboardInterrupt,SystemExit)):
            original=error
            raise
        original=LoopbackTlsFixtureError('loopback TLS fixture generation failed')
        raise original from None
    finally:
        if original is not None:
            try:reap_owned_probes()
            except BaseException as cleanup:
                original.add_note('owned TLS fixture cleanup retry failed: '+type(cleanup).__name__)
