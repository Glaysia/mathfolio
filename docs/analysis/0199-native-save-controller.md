# 같은 MCDX에 저장하는 문서 흐름

r209 실패 이력을 보존하고 새 r210의 고정 양 OS 검증을 완료했다. 직전 채택은 [r208 문단 편집](0197-rich-paragraph-source.md)이다. GUI·스크린샷·설치 Mathcad 실행을 사용하지 않는다. 모든 저장 회귀는 새로 생성한 임시 파일에서 수행한다.

## 원본과 기존 차이

[정적 REA](../../artifacts/rea/native-save-next-20261020.md)는 설치 CIL25개 body를 고정했다. 원본 Ctrl+S는 쓰기 가능한 기존 문서 경로에 저장한다. 새 문서·읽기 전용 문서에는 이름을 묻는 분기가 있다. 원본의 temp→SetLength(0)→CopyTo를 원자적 파일 교체라고 설명하지 않는다.

기존 Mathcraft는 native import 뒤 path를 비워 자체 JSON 저장으로 보냈으며 MCDX 출력은 create-new 복사만 허용했다. 이는 이전 안전 정책이고 원본 저장 흐름과는 다르다. path만 복원하면 MCDX 이름에 자체 JSON을 쓰게 되므로 serializer와 목적지를 함께 연결해야 한다.

## 구현

session의 native target은 경로와 실제 파일 bytes의 길이/SHA revision을 보관한다. native open 때 이미 읽은 bytes로 만들며, own open·새 문서·성공한 own 저장 때 제거한다. source 권위인 native_archive와 파일 저장 목적지는 분리한다. 반복 저장 후 archive만 새 출력으로 갈아끼워 rich wrapper/catalog/package·region/result anchor를 깨뜨리지 않는다.

`prepare_mcdx`는 기존 validate→package→export_into 전체 순서·결과 proof·Pending 정책을 재사용하고 ZIP 출력 길이를16MiB로 제한한다. Vec·압축기·전체 프로세스 메모리의 한도를 뜻하지 않는다. 기존 export_mcdx의 새 파일 복사·기존 목적지 거절 계약은 유지한다.

명시적인 현재 native 저장은 예상 revision이 같은 기존 파일만 교체한다. sibling temp에 완전히 쓰고 portable permissions를 적용해 sync/close한 뒤 revision을 다시 확인하고 rename한다. 성공 뒤 target/revision/date/dirty만 갱신한다. 자체 형식은 기존 JSON serializer를 유지한다. 명시 Save As는 선택 경로의 알려진 확장자를 사용하며, 기존 native를 선택했으면 준비 전에 그 파일 revision을 캡처한다. 취소는 source·draft·history·정체성을 바꾸지 않는다.

symlink/reparse 경로·읽기 전용·외부 내용 변경·미지원 source/출력 크기는 실패한다. 실패한 준비·쓰기·충돌에서 기존 파일을 먼저 지우거나 truncate하지 않는다. 읽기 전용 Ctrl+S의 원본 Save As 재질문은 아직 연결하지 않아 오류로 끝난다. check→rename 사이 동시 수정은 portable atomic compare-and-swap로 막을 수 없으며 custom ACL/owner·네트워크 파일시스템·directory crash durability의 동일성은 주장하지 않는다. Unix hardlink는 이름 하나만 교체하고 다른 링크는 예전 내용을 유지한다.

## 입력 순서 회귀

[독립 코드 검토](../../artifacts/rea/native-save-review-20261020/REPORT.md)는 원본 archive가 없는 일반 Text에서 같은 raw batch의 Text→Ctrl+S가 역전되는 기존 문제를 찾았다. App이 workspace TextEdit보다 먼저 Save를 처리해 이전 source를 저장한 뒤 앞선 글자를 반영했다. native import Text는 이미 RichText queue를 사용하므로 이 재현 범위와 다르다.

[cold 파일 기준 수정 전 실패](../../artifacts/rea/native-save-r209-mixed-batch-cold-before-20261020.log)를 보존했다. 실제 raw hook→전체 app_ui_frame에서 own 문서와 최초 native Save As 뒤에도 archive가 없는 문서를 검사한다. 일반 Text queue에 정확한 app 명령 경계를 추가해 앞선 입력→명령→뒤 입력을 FIFO로 전달한다. Backspace/Delete/Enter/Tab만 있는 일반 편집은 나누지 않으며 기존 IME 경계·Rich/Plot 분기·새 RawInput metadata를 유지한다.

## 개발 검사와 제한

[문서 계층6검사](../../artifacts/rea/native-save-r209-document-preflight-20261020.log)는 Windows에서 반복 교체·opaque part/UTF-16 wrapper·Pending payload·외부 충돌/읽기 전용·늦은 변경·정확한 ZIP 상한을 확인했다. Unix symlink와 권한/hardlink의2개 추가 검사는 Linux 후속 gate 대상이다. cap-minus-one의 의도된 실패에 ZIP writer Drop 진단이 출력되며 test failure는 아니다.

첫 앱 fixture는 minimal presentation에 catalog가 없어 editable source admission이 실패했다. [초기 실패](../../artifacts/rea/native-save-r209-app-preflight-20261020.log)와 [진단](../../artifacts/rea/native-save-r209-app-diagnostic-preflight-20261020.log)을 보존했다. 테스트 fixture에 명시적인 알려진 catalog를 넣은 뒤 [앱6검사](../../artifacts/rea/native-save-r209-app-catalog-fixture-repair-preflight-20261020.log)가 통과했다. 제품 admission을 느슨하게 만들거나 새 텍스트의 원본 기본값을 추정하지 않았다. 원본 catalog가 없는 새 plain Text의 native cold rich 편집은 별도 미구현 범위다.

[혼합 입력 수정 후 검사](../../artifacts/rea/native-save-r209-mixed-batch-after-20261020.log)와 [queue23검사](../../artifacts/rea/native-save-r209-queue-preflight-20261020.log)가 통과했다. [앱 관련14검사](../../artifacts/rea/native-save-r209-all-app-preflight-20261020.log)는 문단/서식/Undo의 반복 native 저장, own/native 전환, IME draft·취소·실패를 포함한다. [Clippy](../../artifacts/rea/native-save-r209-clippy-preflight-20261020.log)도 통과했다. native8개/own2개·벡터 PDF8개의 출력과 Save As 뒤 실제 이전 live target의 불변 관측은 아래 고정 검사와 독립 파일 검증에 포함된다. 파일 출력 성공이나 테스트 수를 원본 전체 동일성·40% 완성으로 환산하지 않는다.

첫 동결 r209의 [Windows 전체 검사](../../artifacts/rea/native-save-r209-windows-tests-20261020.log)와 [peets 전체 검사](../../artifacts/rea/native-save-r209-failed-peets-check-20261020.log)는 기존 native open 비교2개에서 실패했다. 이전 독립 opener의 native pathNone 정책과 새 native 저장 경로Some을 같다고 요구하던 비교다. 원래 독립 opener 코드는 그대로 보존하고, 비교부에서 이 의도된 차이를 명시적으로 검사하도록 고쳤다. native는 기존None/새실제경로+원문 revision, own은 경로 동일/native targetNone을 확인한다. 문서·색상·표·결과 bytes·Pending·계산 상태 등 나머지 독립 비교는 그대로 유지한다. r209 archive/log를 덮어쓰지 않고 새 r210 소스로 양 OS 전체 검증을 다시 수행한다.

비교부의 첫 수정은 실패한 native open에도 성공 경로Some을 요구해 [추가 사전 검사](../../artifacts/rea/native-save-r210-legacy-identity-preflight-20261020.log)의 오류 경로1개가 실패했다. 성공한 native open에만 새 경로/revision 계약을 적용하고 실패·own 경로는 기존 비교와 targetNone을 유지했다. [수정 후 독립 비교3개](../../artifacts/rea/native-save-r210-legacy-identity-success-preflight-20261020.log)는 모두 통과했다. 제품 open/save 동작을 바꿔 이 검사를 통과시킨 변경은 아니다.

## XML 검증 범위 오류와 수리

r210의 [첫 XML 검증](../../artifacts/rea/native-save-r210-xml-validator-20261020.log)은 불변 문단 utility가 과거 전체 inventory152MCDX/10XML/80PDF를 요구해 실패했다. 제품이나 기존 utility를 바꾸지 않고 새 hyphen wrapper가 정확한 과거 inventory를 투영하도록 수리했다. 두 OS의 새 native-save MCDX16개와 PDF16개만 각 목록에서 제외하고, 남은 목록이 과거 inventory와 정확히 같은지 검사한다. 이는 문단 utility 호출 범위이며, 전역 XSD/파일 검사와 native-save utility는 새 파일을 포함한 전체168MCDX/10XML/96PDF를 검증한다.

[수리 후 XML 검증](../../artifacts/rea/native-save-r210-xml-validator-repaired-20261020.log)과 [전체 XML 보고서](../../artifacts/rea/native-save-integration-20261020-r210/xml-validation.json), [문단 범위 보고서](../../artifacts/rea/native-save-integration-20261020-r210/paragraph-xml-validation.json), [native-save 보고서](../../artifacts/rea/native-save-integration-20261020-r210/native-save-xml-validation.json)를 보존했다. 첫 오류와 수리 이력은 validation.json의 validator_prior_failures에, r209 실패 archive/log는 final_gate_retries에 남아 있다. 실패 gate나 validator가 없었다는 뜻이 아니다.

## 양 OS 고정 검증

[최종 검증](../../artifacts/rea/native-save-integration-20261020-r210/validation.json)은 checkpoint_accepted/current_product_matches_archive 모두 true다. 아래 수는 실제 최종 로그에서 집계한 값이다. 개발 중 fixture/입력 순서 실패는 별도 보존하며 원본 GUI·설치 Prime 재열기를 검증했다고 주장하지 않는다.

| 항목 | 결과 |
|---|---|
| 이전 동결 | r209 rejected · archive SHA `9709ca47bf6908d9041844f00a8101bfc950935abf2310ba1d98537b0ea6ff92` · 양 OS 비교 실패 기록 보존 |
| XML validator 수리 | 첫 범위 오류 기록 보존 · 제품 변경 없음 · 과거 subset 투영 + 전체168/96 검증 |
| source archive | 14,110,720 bytes · SHA `4eee9e59e4e7cdb8ebccf3903cf67c8b14a07ffac4eae8a2730c1ea3357e3466` |
| source manifest | 570 files / Rust304 · SHA `fc5203cdce517357f63a2ec59097429d5267f144431fcd3e92d29a492dc45036` |
| Windows / peets 정상 검사 | 1754 / 1756 PASS · 실패0 · Unix 전용 symlink 및 권한/hardlink2개 차이 |
| ignored | Windows94 / peets93 · Windows 전용 headless 그래픽 조회1개 차이 |
| format / Clippy / build / reference | 양 OS PASS · Windows release PASS |
| MCDX / XML / PDF | 168 / 10 / 96 · XSD1018parts · image0 · fitz+pypdf PASS |
| native Save 기능 출력 | OS별 MCDX8 / own2 · revision/source/관계/cache/rich provenance 독립 검증 |
| 실행 파일 | [r210](../../artifacts/rea/mathcraft-native-save-r210-20261020.exe) · 25,466,880 bytes · SHA `def0cff34932e0bb4844770487f949f4a24e1d052902f1b2471f27eea60bf517` |
| 고정 headless harness | 34,953,216 bytes · SHA `370228ffab80ff00b9e5c76f16a6dd9b8892000467f10dcc98d09d9bf7c37916` |
| peets evidence archive | 6,215,680 bytes · SHA `1b72d6496f344ed19ec53fb695851a4405b039916e11962ce9e86c40cb2a2e5b` |
| validation | SHA `c7700771836c288465340e4095099d406a50f57aee3d64b0c5f0ca26691b1c22` |

정상 검사 수의 Unix2개 차이와 Windows-only ignored1개는 서로 다른 조건이다. 플랫폼 수를 같게 만들기 위해 검사를 누락하지 않는다. 원본 cached metadata의 모든 형식·쓰기 권한/네트워크/ACL·동시 교체 race·새 rich Text는 미완료다. D11/HD009의 지원 깊이만 갱신하며 세부555행은 부분156/미구현399, 상위87행은 구현4/부분69/미구현14다. 이를 완성률이나40% 목표 달성으로 환산하지 않는다.
