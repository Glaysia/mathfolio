# 새 Text 생성·서식·저장 연결

2026-10-20 기록. [r210](0199-native-save-controller.md)을 보존하고 r211 고정 양 OS 검증을 완료했다. 아래 개발 관측과 최종 gate를 구분하며 검사 수·부분 구현 깊이를 40% 완성이나 체감 지연 개선으로 환산하지 않는다. GUI·스크린샷·설치 Mathcad·CLR/WPF를 실행하지 않았다.

## 원본 계약

[초기화 조사](../../artifacts/rea/rich-text-initialization-next-20261020.md)는 Ctrl+T가 현재 TextFormat·언어를 복제해 빈 Text owner를 즉시 생성·활성화하는 경로를 보존한다. 세 layout key만 Normal로 바꾸며 실제 글꼴·크기·색상을 일괄 초기화하지 않는다. [후속 빈 문서 조사](../../artifacts/rea/rich-text-new-authoring-next-20261020.md)의 WPF 최초 FlowDocument와 이후 Prime 저장 직렬화는 구분한다. Report 템플릿의 저장된 빈 Run을 Ctrl+T 직후 원본 출력으로 간주하지 않는다.

[지속형 크기 계약](../../artifacts/rea/rich-text-catalog-size-next-20261020.md)은 실제 collection/serializer/property converter를 연결한다. 일반 TextStyle의 font-size는 DIP이고, 정확히 areaLayoutStyle/headerLayoutStyle/footerLayoutStyle은 TextFormat의 points 속성이다. 변환은 확인된 Double 연산 순서 `(points / 72.0) * 96.0`를 따른다. 이름만 보고 모든 font-size를 같은 단위로 해석하지 않는다. 원본 reader의 누락 style-key=name 보완도 이 세 layout 항목에 한해 사용한다.

## 문서와 앱

새 Text는 imported `RichTextWire`와 다른 `AuthoredTextWire`를 사용한다. 명시적인 현재 서식·언어·resolved catalog와 seed SHA를 보관하며 원본 native anchor를 위조하지 않는다. 자체 scaffold는 Mathcraft 구현 정책이며 Prime의 최초 빈 저장 XML과 byte 동일하다는 주장이 아니다. 원본 native carrier의 JSON·authority는 그대로다.

Canvas carrier는 Text/source_region=None에만 허용하고 두 carrier의 동시 존재를 거절한다. 불신 자체 파일은 seed·source·plain을 다시 검증한다. 전체 직렬화 wire와 plain을 owner마다 8 MiB 문서 예산에 합산하며 공유 Arc를 이유로 비용을 할인하지 않는다. byte/fingerprint 캐시는 serde에 저장하지 않고 authority로 취급하지 않는다.

Ctrl+T와 첫 Text 입력은 source/layout/예산을 먼저 준비한 뒤 owner·focus·이력 하나를 게시한다. 실패하면 ID·문서·선택·이력을 바꾸지 않는다. 가져온 문서는 검증된 body layout/Normal과 파일 언어를 사용한다. 새 자체 문서는 원본 TextFormat fallback appearance와 한국어 proofing을 명시적인 Mathcraft 기본 정책으로 사용한다. 지원하는 authored Run의 현재 서식은 작은 snapshot으로 보관하고 새 Text 명령에서만 seed를 생성한다. mixed toolbar 값이나 수학용 region.format에서 현재 Text 서식을 추측하지 않는다.

공통 TextEdit/IME publication 경로에 두 source 종류를 연결했다. 조합 draft는 source/history에 게시하지 않고 확정된 Run 치환만 적용한다. 새 authored source는 빈 문단·시작/끝·연속 Enter를 지원한다. B/I·크기·foreground는 비어 있지 않은 선택에 적용하며 source·문서 Undo/Redo·영역 복사·자체 저장·native 출력·벡터 표시를 연결한다. 원본 imported 문단의 제한된 분할 정책은 별도이며 이 변경으로 자동 확대하지 않는다.

## 실제 native 출력

[브리지 v3](../../artifacts/rea/rich-text-authored-export-v3-next-20261020/README.md)는 기존 nested ZIP builder와 part/관계 allocator를 재사용한다. 실제 새 owner binding을 private package에 잠시 설치해 기존 authority reader로 cold source를 증명한 뒤 원래 worksheet bytes를 복원한다. 최종 owner는 기존 outer projection이 한 번만 추가한다. 저장 후 loaded native_archive를 재기준화하지 않는 r210 정책을 유지한다.

원본 catalog의 기존 key는 알려진 명시 appearance와 허용된 빈 based-on·None marker·Normal variants·0 indent일 때만 재사용한다. 실제 LCC의 custom display name Normal과 별도 localized Normal key를 혼동하지 않는다. unknown/inherited/conflicting 정의를 덮어쓰지 않는다. 세 layout의 누락 key alias 외 기존 catalog bytes는 보존하며 새 textRendering을 만들 때만 seed의 ProofingLanguage를 기록한다.

[혼합 순서 수정](../../artifacts/rea/rich-text-authored-projection-order-next-20261020/README.md)은 imported format-only receipt와 새 authored catalog가 함께 있을 때의 저장 실패를 다룬다. authored가 있을 때만 기존 catalog 권위에서 deferred format을 먼저 적용하고 전체 context/Pending 판정으로 보낸다. binding/hash 증명은 완화하지 않는다. authored가 없는 기존 inert 경로는 그대로다. 이전 순서를 실제 함수로 재현하는 실패 oracle과 다중 COW·충돌 거절 회귀를 보존한다.

## 입력 비용과 빈 줄

[독립 앱 검토](../../artifacts/rea/authored-app-review-20261020/REPORT.md)의 세 수정을 적용했다. 빈 Run의 크기·색상을 empty job/문단 row/마지막 글자 삭제 preview에 반영한다. source 검증 뒤 표시 불가인 서식은 정확한 owner/version/plain에 오류를 묶어 idle 때 재검증하지 않는다. 확정된 source의 시각 갱신은 retained validated source를 재사용한다. readonly Flow XML 파싱과 LayoutJob 문자열 복사 등 남은 비용은 있다.

[scalar index 수정](../../artifacts/rea/rich-text-scalar-index-next-20261020/README.md)은 매 Run마다 문단 접두부의 글자 수를 다시 세던 경로를 누적 Unicode scalar counter로 바꾼다. 원래 parser를 고정한 전체 index/오류 비교와 별도 Unicode/entity/빈 문단 oracle을 사용한다. [개발 CPU 관측](../../artifacts/rea/authored-r211-scalar-observation-20261020.log)은 8회 합계로 생성 4096-Run 사례 91.132→56.7753 ms, 실제 sample01 0.1536→0.1659 ms였다. 작은 표본의 악화도 보존한다. 순서가 고정된 단회 test-profile 관측이며 release 입력 지연·GPU·RSS 개선의 증거가 아니다.

[NativePart 공유](../../artifacts/rea/native-part-sharing-next-20261020/README.md)는 private payload를 `Arc<Vec<u8>>`로 보관한다. open/set에서 기존 Vec buffer를 이동하고, package clone은 불변 payload를 공유하며 변경 part만 교체한다. 공개 slice·DTO·ZIP writer·검증/교체 순서는 그대로다. old Vec 기반 writer와 출력 ZIP bytes 및 독립 reopen metadata를 비교한다. Arc header/refcount·metadata 복사 비용은 남는다. [같은 LCC 읽기 전용 관측](../../artifacts/rea/authored-r211-native-clone-observation-20261020.log)은 39 parts/171249 payload bytes에서 32 clone 합계 old Vec 2.3353 ms, candidate 0.0778 ms였다. candidate cold parse 1.7236 ms는 baseline cold와 비교한 값이 아니다. 정적 payload copy 제거와 단회 warm 관측을 전체 앱 지연 개선률로 환산하지 않는다.

## 개발 검사

- [core 최초 57 검사](../../artifacts/rea/authored-r211-core-preflight-20261020.log), [scalar/shared query 61 검사](../../artifacts/rea/authored-r211-shared-query-preflight-20261020.log).
- [앱 텍스트 164 정상·5 ignored 검사](../../artifacts/rea/authored-r211-native-app-preflight-20261020.log): 실제 raw hook/app frame, IME·Ctrl+T·current format·history·copy·cold reopen·기존 native 텍스트 회귀 포함.
- [native 문서 359 정상·8 ignored 검사](../../artifacts/rea/authored-r211-native-document-iterator-repair-20261020.log). 이후 [혼합 projection 3 검사](../../artifacts/rea/authored-r211-mixed-projection-preflight-20261020.log)를 별도로 통과했다. 이를 현재 전체 gate 합계라고 부르지 않는다.
- [공유 payload 적용 뒤 전체 문서 367 정상·9 ignored 검사](../../artifacts/rea/authored-r211-shared-package-mode-fixture-repair-20261020.log)와 [workspace/all-target Clippy](../../artifacts/rea/authored-r211-workspace-clippy-wire-repair-20261020.log)를 통과했다. 이 항목은 개발 당시 검사이며 최종 양 OS 동결 gate는 아래에 별도로 기록한다.
- [앱 packet producer](../../artifacts/rea/authored-r211-packet-preflight-20261020.log): empty/typed/styled/paragraph/undo/redo 각각 자체 파일·MCDX·벡터 PDF를 생성하고 native cold 원문·계산 미실행을 확인했다. [독립 파일 검사](../../artifacts/rea/authored-validator-next-20261020/actual-preflight-c.json)는 6개씩 실제 파일의 seed SHA·source/plain·live 관계·catalog·DIP·언어·Undo/Redo와 두 PDF parser·images0을 확인했다. 이것은 개발 packet이며 고정 양 OS gate 전이다.

첫 앱 검사에서는 IME test field 이름 오류와 XML 속성 순서에 의존한 잘못된 test assertion이 있었다. 각각 [컴파일 실패](../../artifacts/rea/authored-r211-app-first-preflight-20261020.log), [첫 assertion 실패](../../artifacts/rea/authored-r211-app-preedit-field-repair-20261020.log)를 보존했다. 테스트는 실제 Run 요소의 속성/문자열을 검사하도록 고쳤다. 캐시 도입 후 1024문단 fixture가 이전 private wire의 캐시를 붙인 채 XML을 바꿔 [문서 검사](../../artifacts/rea/authored-r211-document-preflight-20261020.log)가 실패했다. fixture를 실제 cold JSON 경로로 고쳤으며 source proof는 완화하지 않았다. native 브리지의 Iterator receiver 컴파일 오류도 [첫 로그](../../artifacts/rea/authored-r211-native-document-preflight-20261020.log)에 남겼다.

공유 payload의 [첫 문서 검사](../../artifacts/rea/authored-r211-shared-package-preflight-20261020.log)는 Windows ZIP fixture가 요청한 Unix mode와 실제 저장 mode를 같다고 가정한 마지막 assertion에서 실패했다. old Vec writer와 출력 bytes·전체 metadata 비교는 이미 같았다. 기대값을 입력 ZIP에 실제 저장된 mode로 독립 취득하도록 고쳤고 제품 permission 동작은 바꾸지 않았다. [첫 Clippy](../../artifacts/rea/authored-r211-workspace-clippy-preflight-20261020.log)의 큰 Wire enum은 최종 Canvas 저장에 필요했던 native Box를 결과 단계에서 만들도록 조정했다. native allocation 수를 추가하지 않고 warning을 해결했다.

## 남은 범위

원본 새 빈 XML serializer 동일성, imported Text의 빈 문단/시작·끝 Enter 확대, 문단 병합·LineBreak·cross-Run 선택 치환, collapsed word/springload, 모든 inherited catalog/current native format, 밑줄·글꼴 선택·Text Block·그림/임베디드 저작은 미완료다. 새 텍스트 또는 catalog 변경은 전체 native context 증명에 따라 보수적인 Pending을 만들 수 있다. 원본 런타임으로 generated MCDX를 여는 검증과 실제 화면 지연 측정은 하지 않았다. 40% 목표는 계속 진행 중이다.

## 양 OS 고정 검증

아래 수치는 accepted `validation.json`과 SHA로 결합된 실제 완료 보고서에서 읽었다. 개발 preflight·검증기 수리·이전 checkpoint 이력과 구분한다.

| 항목 | 고정 근거 |
|---|---|
| Windows / peets 정상 검사 | 1829 / 1831 |
| Windows / peets ignored | 97 / 96 |
| 원본 구조·XSD | 180 MCDX + 별도 10 XML, 1090 parts |
| 벡터 PDF | 108, 두 parser 구조 검사 및 이미지 0 |
| authored own / observation | 12 / 2 |
| source 파일 / product Rust | 590 / 321 |
| archive SHA256 | `fa18f3eab51cc09a4bd3cb6802664e04288e46d70acd4a7b52e86611a7fd6eee` |
| manifest SHA256 | `d45e475004b6f304596d520d337f232bee156480cb19d2f0e1319f1b71974637` |
| accepted validation SHA256 | `7168d436b6a93122f7ae502160702a218f499ec0ddda93481ec2fed57f7f7f65` |
| mathcraft-authored-text-r211-20261020.exe | 25741824 bytes, `9a5fc64450f1b42786c1ad285d46769ef025055d12af4658e2670e16fb7bfbb1` |
| authored-text-r211-headless-20261020.exe | 35492864 bytes, `d88e7c95ce0f8e0d067cf54ca9e5d766c37da4b22df845680caff41061d90168` |
| authored-text-r211-peets-evidence.tar | 6440960 bytes, `d8c420c1a67a3a66445516028f728f20bfd1fe06849a6ef70eedc053ffd470ac` |

보고서는 `artifacts/rea/authored-text-integration-20261020-r211/validation.json`, `xml-validation.json`, `pdf-validation.json`, `authored-text-{xml,pdf}-validation.json`이다. 독립 v2 검사는 live relationship의 실제 XAML·seed/plain/source, opaque foreground RGB·DIP→point를 PDF scalar별 span과 대조한다. NotoEmoji의 epaint 0.81 source scale은 별도로 적용하며 bold font명·빈 Run PDF 서식·원본 글꼴/픽셀 parity는 주장하지 않는다. UI/원본 재열기·GPU·입력 지연·40% 달성의 증거가 아니다.

검사 수는 재실행 횟수를 합산한 수가 아니다. `final_gate_retries`, `validator_prior_failures`와 아래 추가 정규화/재시도 metadata를 accepted receipt 그대로 보존한다. 기능별 상태 이동 없이 현재 요약 표의 오래된 합계만 실제 catalog와 맞췄다.

```json
{
  "validator_prior_failures": [
    {
      "file": "native-save-r210-xml-validator-20261020.log",
      "sha256": "af8935bdae2e60c7b8b56f8720446a1e2df3e4e3f8caf1e629f95a61b427e059",
      "cause": "frozen paragraph utility expected historical152/10/80 full inventory; new wrapper projects exact old inventory and global/native validators retain all168/10/96",
      "product_changes": false
    },
    {
      "file": "r211-testset-binder-interleave-v2-next-20261020/prior/failure-receipt.json",
      "sha256": "6129cd1b20d688f7c68ec1de564384dab1da0d2050c4f398d2cbadade2523777",
      "checkpoint": "authored-text-r211-metadata",
      "product_changes": false,
      "cause": "frozen ZIP cap negative-test stderr interleaved libtest stdout: prior line-anchored parser omitted two actual document ok names; exact known diagnostic normalization preserves stdout ok/newline and rejects unknown interleaves",
      "raw_gate_log_file": "r211-testset-binder-interleave-v2-next-20261020/prior/authored-text-r211-check.log",
      "raw_gate_log_sha256": "fd49978b3bf2d60d2d4675ad603c4a70d5b264de06fcb1f0dc9fb40e4265c2b1",
      "prior_utility_file": "r211-testset-binder-interleave-v2-next-20261020/prior/authored_testset_r211.py",
      "prior_utility_sha256": "4b827b1339060f9a7835bfbaff03a78ff1d512d1cff29a5968ee1bca7c6ca094",
      "limitation": "receipt reproduces pure parsing failure; parent original console error was not available as a separate retained log"
    },
    {
      "file": "r211-testset-binder-stream-v3-next-20261020/prior/failure-receipt.json",
      "sha256": "44b802d2b35084a3bdd8a980a0abd706480789a552f48cce3b0449beaacf2494",
      "checkpoint": "authored-text-r211-metadata",
      "product_changes": false,
      "cause": "Cargo solve Running header on stderr arrived before scientific stdout summary; prior parser incorrectly terminated the scientific block. Separate ordered stderr identifiers and stdout running/result blocks now require exact names/counts and fresh enumeration equality, including zero harnesses/doctests",
      "raw_gate_log_file": "r211-testset-binder-stream-v3-next-20261020/prior/authored-text-r211-windows-tests-20261020.log",
      "raw_gate_log_sha256": "696e96b0ad9101a083616808b1ef68b8a3433d46a063210984cecbc0a411e612",
      "prior_utility_file": "r211-testset-binder-stream-v3-next-20261020/prior/authored_testset_r211.py",
      "prior_utility_sha256": "096de69351b6870ce6b9e5ecf72fbc077dd20bf453a47743b5b8744966fa46b8",
      "limitation": "receipt reproduces pure parsing failure; parent original console error was not available as a separate retained log"
    },
    {
      "file": "authored-text-r211-finalization-20261020.log",
      "sha256": "5c12a6a6526f9756f3596f02fb9a94d94ced3736965fb17f63b111a1a1ecd8d1",
      "checkpoint": "authored-text-r211-finalizer",
      "product_changes": false,
      "cause": "v2 fresh PDF run_scalar_coverage has integer Run ordinal object keys; saved JSON has string keys. Fresh semantic report JSON roundtrip is exactly equal. Only finalizer wire comparison is normalized; immutable v1/v2 validators and product source unchanged.",
      "prior_finalizer_file": "authored-r211-finalizer-before-json-roundtrip-20261020.py",
      "prior_finalizer_sha256": "1cbfc8da1e17bb2af17b957de90eb28b15b7a83755d1d347044a81c458576e7f"
    }
  ],
  "final_gate_retries": [
    {
      "checkpoint": "native-save-r209",
      "accepted": false,
      "archive_sha256": "9709ca47bf6908d9041844f00a8101bfc950935abf2310ba1d98537b0ea6ff92",
      "failure": "both OS tests rejected intentional native path retention against old copy-only pathNone policy",
      "followup": "legacy opener body unchanged; comparison explicitly verifies successful native path/revision delta, own/failure still retain legacy path behavior; all other document/visual/cache/calculation comparisons remain exact",
      "r210_changed_files": [
        "crates/mathcraft-app/src/worksheet_ui/tests/native_open_tests.rs",
        "docs/analysis/0199-native-save-controller.md"
      ],
      "evidence": [
        {
          "file": "native-save-r209-windows-driver-20261020.log",
          "sha256": "d4ed043ff75607dae03a5b1e103ef597b22038ba4150d2d866cdc0a56a2b692f"
        },
        {
          "file": "native-save-r209-windows-tests-20261020.log",
          "sha256": "f10b06e55395a971e1023c317bd1779f2eadb47f0b03ce069a7addacbfaeda58"
        },
        {
          "file": "native-save-r209-peets-driver-20261020.log",
          "sha256": "9856121614ae0d3ed50bcc28a873ac3c21202329abeef50b9146131aade499ad"
        },
        {
          "file": "native-save-r209-failed-peets-check-20261020.log",
          "sha256": "ac541353663f95b8551d1ad356d23cf749ee3e8ace481b4ab9a3bd547860de53"
        },
        {
          "file": "native-save-r210-legacy-identity-preflight-20261020.log",
          "sha256": "77b4e5b8e133002471f6f8d29d91f3d045ccb46ee8eb43495704b7bfbf4d4cd3"
        },
        {
          "file": "native-save-r210-legacy-identity-success-preflight-20261020.log",
          "sha256": "5887884a4131777f8ad13d8126032280efdeb6455d513742e4e224c76d527352"
        }
      ]
    }
  ]
}
```
