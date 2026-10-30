"""Build the shared Rust editor SDK. Output stays in ignored dist/wasm-sdk.

Prerequisites: rustup target add wasm32-unknown-unknown; wasm-bindgen-cli matching
Cargo.lock; npm ci --prefix web. No Node/JS runtime is added to native Mathfolio.
"""
from pathlib import Path
import argparse, gzip, hashlib, json, shutil, subprocess, tarfile, tomllib

ROOT = Path(__file__).resolve().parents[1]
def run(args, **kwargs):
    subprocess.run([str(a) for a in args], cwd=ROOT, check=True, **kwargs)
def main():
    parser=argparse.ArgumentParser(); parser.add_argument('--skip-build',action='store_true'); parser.add_argument('--skip-opt',action='store_true'); args=parser.parse_args()
    cargo=shutil.which('cargo') or str(Path.home()/'.cargo/bin/cargo.exe')
    bindgen=shutil.which('wasm-bindgen') or str(Path.home()/'.cargo/bin/wasm-bindgen.exe')
    node=shutil.which('node')
    expected=next(p['version'] for p in tomllib.loads((ROOT/'Cargo.lock').read_text(encoding='utf-8'))['package'] if p['name']=='wasm-bindgen')
    actual=subprocess.check_output([bindgen,'--version'],text=True).strip()
    if actual != 'wasm-bindgen '+expected: raise SystemExit(f'Install matching tool: cargo install wasm-bindgen-cli --version {expected} --locked')
    out=ROOT/'dist/wasm-sdk'; out.mkdir(parents=True,exist_ok=True)
    version=tomllib.loads((ROOT/'Cargo.toml').read_text(encoding='utf-8'))['workspace']['package']['version']
    raw=ROOT/'target/wasm32-unknown-unknown/release/mathfolio_ui.wasm'
    if not args.skip_build: run([cargo,'build','--locked','--release','-p','mathfolio-app','--lib','--target','wasm32-unknown-unknown'])
    run([bindgen,raw,'--target','web','--out-dir',out,'--out-name','mathfolio'])
    # Remove only obsolete generator-owned files from the pre-rename SDK.
    for legacy in ['mathcraft.js','mathcraft.d.ts','mathcraft_bg.wasm','mathcraft_bg.wasm.d.ts']:
        (out/legacy).unlink(missing_ok=True)
    bound=out/'mathfolio_bg.wasm'; before=bound.read_bytes()
    # Separate candidate so a failed optimization never replaces a valid module.
    optimized=ROOT/'target/wasm32-unknown-unknown/release/mathfolio-optimized.wasm'
    flags=['-O2','--enable-bulk-memory','--enable-nontrapping-float-to-int','--enable-sign-ext','--enable-reference-types','--enable-multivalue']
    meta=optimized.with_suffix('.json')
    key={'input_sha256':hashlib.sha256(before).hexdigest(),'binaryen':json.loads((ROOT/'web/node_modules/binaryen/package.json').read_text(encoding='utf-8'))['version'],'flags':flags}
    cached=json.loads(meta.read_text(encoding='utf-8')) if meta.exists() else {}
    reusable=optimized.exists() and cached.get('key')==key and cached.get('output_sha256')==hashlib.sha256(optimized.read_bytes()).hexdigest()
    if not args.skip_opt and not reusable:
        run([node,ROOT/'web/node_modules/binaryen/bin/wasm-opt',bound,*flags,'-o',optimized])
        meta.write_text(json.dumps({'key':key,'output_sha256':hashlib.sha256(optimized.read_bytes()).hexdigest()},indent=2),encoding='utf-8')
    after=before if args.skip_opt else optimized.read_bytes()
    if len(after)<len(before): bound.write_bytes(after)
    for file in ['sdk.ts','tsconfig.json','worker.js']: shutil.copy2(ROOT/'web'/file,out/file)
    run([node,ROOT/'web/node_modules/typescript/bin/tsc','--project',out/'tsconfig.json'])
    for file in ['sdk.ts','tsconfig.json']: (out/file).unlink()
    shutil.copy2(ROOT/'LICENSE',out/'LICENSE')
    shutil.copy2(ROOT/'NOTICE.md',out/'NOTICE.md')
    shutil.copytree(ROOT/'assets/fonts/stix',out/'licenses/stix',ignore=shutil.ignore_patterns('*.ttf'),dirs_exist_ok=True)
    shutil.copytree(ROOT/'assets/themes/nord',out/'licenses/nord',dirs_exist_ok=True)
    shutil.copytree(ROOT/'assets/fonts/egui-default',out/'licenses/egui-default',dirs_exist_ok=True)
    shutil.copy2(ROOT/'web/SDK-README.md',out/'README.md')
    # CJK is external to WASM: applications can replace it via provideFont.
    font=ROOT/'assets/fonts/nanum';
    if font.exists(): shutil.copytree(font,out/'fonts/nanum',dirs_exist_ok=True)
    (out/'package.json').write_text(json.dumps({'name':'@mathfolio/wasm-sdk','version':version,'type':'module','license':'AGPL-3.0-only','exports':{'.':{'types':'./sdk.d.ts','default':'./sdk.js'},'./bindings':'./mathfolio.js','./mathfolio_bg.wasm':'./mathfolio_bg.wasm','./worker.js':'./worker.js'}},indent=2)+'\n',encoding='utf-8')
    # Reports live outside the package, so repeated builds cannot archive a stale report.
    if (out/'size-report.json').exists(): (out/'size-report.json').unlink()
    files={str(p.relative_to(out)).replace('\\','/'):{'bytes':p.stat().st_size,'sha256':hashlib.sha256(p.read_bytes()).hexdigest()} for p in sorted(out.rglob('*')) if p.is_file()}
    report={'raw_rust_wasm_bytes':raw.stat().st_size,'bindgen_wasm_bytes':len(before),'optimized_candidate_bytes':len(after),'shipped_wasm_bytes':bound.stat().st_size,'wasm_gzip_bytes':len(gzip.compress(bound.read_bytes(),mtime=0)),'package_uncompressed_bytes':sum(f['bytes'] for f in files.values()),'files':files}
    archive=ROOT/f'dist/mathfolio-wasm-sdk-{version}.tar.gz'
    with archive.open('wb') as stream:
        with gzip.GzipFile(fileobj=stream,mode='wb',mtime=0) as zipped:
            with tarfile.open(fileobj=zipped,mode='w') as tar: tar.add(out,arcname='package')
    report['package_tar_gzip_bytes']=archive.stat().st_size
    report['optimized']=not args.skip_opt
    report['tools']={'wasm_bindgen':actual,'rustc':subprocess.check_output([str(Path(cargo).with_name('rustc.exe' if Path(cargo).suffix=='.exe' else 'rustc')),'--version'],text=True).strip(),'binaryen':json.loads((ROOT/'web/node_modules/binaryen/package.json').read_text(encoding='utf-8'))['version']}
    report['raw_rust_wasm_sha256']=hashlib.sha256(raw.read_bytes()).hexdigest()
    report['package_sha256']=hashlib.sha256(archive.read_bytes()).hexdigest()
    (ROOT/'dist/wasm-sdk-size.json').write_text(json.dumps(report,indent=2)+'\n',encoding='utf-8')
    print(json.dumps({k:v for k,v in report.items() if k!='files'},indent=2))
if __name__=='__main__': main()
