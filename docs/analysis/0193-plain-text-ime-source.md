# 0193 — 일반 텍스트 조합 입력과 문서 원문

2026-10-19. 직전 채택 기준은 [r204](0192-affine-temperature-integration.md)이다. 이 후속은 일반 Text(String)의 저장 원문·조합 화면·문서 Undo를 분리한다. 원본의 [서식 있는 텍스트 정적 계약](../../artifacts/rea/rich-text-next-20261019.md)과 함께 진행하지만, WPF의 미확정 조합 중 Save/취소 정책을 원본 실행으로 관찰했다는 뜻은 아니다. 전체 Rich Text 저작과 40% 목표는 남아 있다.

## 실제로 확인한 문제

[첫 관찰](../../artifacts/rea/plain-text-ime-red-20261019/observation.json)은 실제 egui TextEdit에 `앞😀뒤`의 첫 글자를 선택하고 Preedit `ㅎ`→`하`→Commit `한`을 전달했다. 조합 중 Save가 `하😀뒤`를 썼으며, 확정 후 문서 Undo도 `하😀뒤`를 복원했다. 취소는 원래 선택했던 `앞`을 잃었다. [수정 전 계약 검사](../../artifacts/rea/plain-text-ime-contract-red-20261019.log)는 세 검사 모두 실패했다. 설치 Mathcad·GUI·스크린샷은 사용하지 않았다.

원인은 TextEdit가 document의 String을 직접 수정하지만, 기존 입력 의도 판별은 Preedit를 원문 변경으로 취급하지 않아 Undo baseline도 준비하지 않은 데 있다. Preedit마다 문서 이력을 추가하는 방식은 사용하지 않는다.

## 수정 경계

[plain_text_input.rs](../../crates/mathfolio-app/src/worksheet_ui/plain_text_input.rs)는 조합 중에만 별도 draft를 만들고, 확정 전 문서 원문과 문서 Undo를 보존한다. Save·인쇄는 확정된 원문을 읽는다. Commit은 문서의 한 편집으로 저장하고, 취소는 조합 전 선택 내용을 유지한다. 수식의 Enter 확정 규칙을 텍스트에 적용하지 않는다. 일반 Enter/Shift+Enter는 줄바꿈이다.

[독립 검토](../../artifacts/rea/plain-text-ime-independent-review-20261019.md)는 한 프레임의 일반 Text·Preedit·Commit·커서 이동을 마지막 상태만으로 판별하면 입력 유실 또는 미확정 글자 유입이 가능하다고 지적했다. 이 지적은 정적 반례이며 수정 전 실행 실패로 포장하지 않는다. 기존 [입력 큐](../../crates/mathfolio-app/src/worksheet_ui/plot_input_queue.rs)의 단일 FIFO로 IME와 일반 입력의 경계를 RawInput 단계에서 보존한다. 일반 입력만 있는 프레임은 나누지 않고, 섞인 경계만 다음 repaint로 순서대로 전달한다. 임의 sleep이나 OS 입력 복제·누락을 넣지 않는다. 주변 문자 삭제는 조합에 의해 밀려난 원래 선택 내용을 분리해 문서에 반영한다.

[실제 이벤트 회귀](../../crates/mathfolio-app/src/worksheet_ui/plain_text_ime_tests.rs)는 저장·취소·Undo/Redo, 같은 batch의 확정 후 다음 조합, 일반 입력과의 순서, 선택 변경, 주변 삭제, 포커스·문서 전환을 확인한다. 관찰용 own 파일은 실제 앱 Save로 생성하고 다시 읽는다. 이 보강의 양 OS 고정 검증을 완료했다. 아래 완료 기록은 별도 고정 소스·로그를 기준으로 하며, 위 최초 관찰과 수정 전 실패를 그대로 보존한다.

문서 전환은 앱의 draft를 지우는 것만으로 충분하지 않았다. 같은 영역 ID로 다른 문서를 열고 빈 Preedit를 받으면 이전 egui 조합 상태가 새 문서의 첫 글자를 지우는 [실패도 재현](../../artifacts/rea/plain-text-ime-reused-id-red-20261019.log)했다. 새 편집 소유자에게 포커스를 줄 때 저장된 현재 커서 또는 새 끝 커서로 깨끗한 widget 상태를 설치한다.

## 남은 공백

텍스트 입력 중 native Run 서식이 소실되는 기존 경로, 선택 범위 서식·기본 입력 서식·스타일 카탈로그 편집은 별도 작업이다. readonly RichDocument를 원본 source로 승격하지 않는다. native lowercase Run `style` 키, 상위 서식, worksheet textStyles와 리소스를 보존해야 한다. 이 수정만으로 D08 전체 구현이나 체감 성능 해결을 주장하지 않는다.

## 최종 고정 소스 검증: r205

[validation.json](../../artifacts/rea/string-values-integration-20261019-r205/validation.json)의 `checkpoint_accepted: true`를 확인한 뒤 이 기록을 작성했다. Windows/peets 각각1619검사와 134MCDX·별도10XML의 XSD 814parts·64PDF 구조 검증, IME 관찰 own 10파일·observation 2개 검증을 통과했다. 검사 수는 이 고정 소스의 새 gate 로그에서 도출하며 ignored를 성공 수에 합산하지 않는다.

| 검증 | 결과 |
|---|---|
| Windows workspace + backend | 1617 + 2 = 1619 |
| peets workspace + backend | 1619 |
| OS별 일반 게이트 ignored | 88 |
| 소스 manifest | 527파일 / 제품 Rust 272파일 |
| native/별도 XML XSD | 134 MCDX + 10 XML / 814parts, 오류0 |
| PDF | 64개 / 구조·두 reader·이미지0 검사 |
| IME 기능 관찰 | OS별 1개 ignored probe를 명시 실행 / 각4시나리오 / own 10개 / observation 2개 |
| archive SHA256 | `72371fe78aa3963dde7cba54ed429fd841317ffaba34e5e8b64b84109e1cea7c` |
| manifest SHA256 | `7c6ffe96762db99e7b396caaca52b9a0767be6aaf8cba1aa105a400439dd5699` |

[IME 검증 JSON](../../artifacts/rea/string-values-integration-20261019-r205/plain-text-ime-validation.json)은 조합 중 Save가 `앞😀뒤`, Commit 후 Save가 `한😀뒤`, 문서 Undo가 `앞😀뒤`, Redo가 `한😀뒤`인지 실제 own 파일과 관찰 JSON으로 대조한다. empty Preedit·empty Commit·Disabled 취소는 원문과 이력을 보존하고 계산 실행 및 submitted generation을 바꾸지 않는다. 한 batch의 일반 문자·조합 경계, 포커스·문서 전환은 해당 고정 소스의 일반 회귀에 포함된다. Save-before-Commit은 확정 원문을 저장하는 이 앱의 선택이며 원본 WPF의 미확정 조합 정책을 회수했다는 주장이 아니다.

| 산출물 | 바이트 | SHA256 |
|---|---:|---|
| [mathcraft-string-values-r205-20261019.exe](../../artifacts/rea/mathcraft-string-values-r205-20261019.exe) | 24955392 | `dcdac13fd6f88425dc500b4762e8d041502df249996aa79b36f55bbe558e6311` |
| [string-values-r205-headless-20261019.exe](../../artifacts/rea/string-values-r205-headless-20261019.exe) | 28217856 | `0e0cab9f120760980666e52e777d9faaad7bf8bcff2046dd38e4315286df438e` |
| [string-values-r205-peets-evidence.tar](../../artifacts/rea/string-values-r205-peets-evidence.tar) | 5376000 | `338683f5139878b160d0868c6fd3799ac44d4d1ae9cae51d57b7abcae6b75758` |

fmt·workspace/backend Clippy·일반 검사·build·reference·Windows release와 명시 기능 probe의 고정 소스 검증이다. CPU timing probe는 이 게이트에서 실행하지 않았다. [0191의 r204 CPU 자료](0191-lcc-workspace-cpu.md)는 별도 측정으로 유지한다. 원본 Rich Text 저작·Run/style 보존과 전체 IME 동작은 여전히 별도 범위이며 기능 목록의 상태·수를 바꾸지 않았다.

후처리 XML 검증기의 첫 실행은 observation 경로 목록의 정렬 차이로 실패했다. [첫 로그](../../artifacts/rea/string-values-r205-xml-validator-20261019.log)와 [당시 검사기](../../artifacts/rea/validate_plain_text_ime_r205-preinventorysort.py)를 보존했다. 정확한 양쪽 경로 목록을 모두 정렬하도록 비교식만 고친 [재검증](../../artifacts/rea/string-values-r205-xml-validator2-20261019.log)이 통과했다. 제품 소스·게이트·probe는 바꾸거나 재실행하지 않았다.
