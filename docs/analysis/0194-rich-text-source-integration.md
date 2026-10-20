# 선택 텍스트 서식과 원본 XAML 보존

r206의 양 OS 고정 검증을 완료했다. 직전 채택 버전은 [r205](0193-plain-text-ime-source.md)이며, 아래 작업이나 테스트 수를 전체 완성도 또는 40% 달성으로 환산하지 않는다. GUI·스크린샷·설치된 Mathcad 실행 없이 정적 REA와 독립 Rust/headless 경로를 사용했다.

## 원본 계약

REA로 조사한 [텍스트 원문/스타일 근거](../../artifacts/rea/rich-text-next-20261019.md)는 설치된 메서드 102개의 식별자와 IL 바이트를 기록한다. [리본/키보드 근거](../../artifacts/rea/rich-text-toolbar-next-20261019.md)는 Prime/component 41개와 설정상 WPF 의존성 5개를 구분한다. 설정에 대응하는 설치된 WPF 분석은 실제 로드·실행 관찰이 아니다. 원본의 선택 글자 서식, 초기 소스의 상속 속성, 이후 catalog 재바인딩을 구별했으며 읽기용 렌더 DTO를 저장 원문으로 사용하지 않는다.

굵게·기울임 리본은 혼합 선택에서 이전 속성 값을 유지한다. Ctrl+B/I는 선택 속성이 정확히 대상 값이면 Normal, 그 외에는 대상 값으로 설정한다. 두 경로를 같은 토글 규칙으로 바꾸지 않는다. 크기는 point 값을 Enter/포커스 이탈/목록 확정 때 적용하며 1–1638 point와 소수 한 자리를 검사한다. source DIP 변환은 원본의 `points / 72 * 96` 순서를 따른다. 색상은 리본 RGBA와 원문 ARGB를 한 번 변환하며 alpha를 보존한다.

원본 텍스트 Enter는 문단 입력이다. 현재의 제한된 rich source 편집에 줄바꿈을 임의의 확정/수식 명령으로 추가하지 않는다. 문단·Run 경계의 전체 편집 규칙이 구현됐다는 뜻도 아니다.

## 연결한 범위

- `CanvasRegion.rich_text`는 선택적 Box carrier다. 없으면 기존 JSON/signature가 바뀌지 않으며, 있으면 실제 보존 MCDX의 고유 Text owner·관계·nested package·catalog·namespace 완성 원래 wrapper SHA에 다시 연결해야 한다. private source/index와 불변 Arc 문자열을 사용하고 wire 역직렬화만으로 편집·출력을 허가하지 않는다.
- 지원하는 Paragraph/Run에서 비어 있지 않은 같은 문단 선택에 굵게·기울임·크기·색상을 적용한다. 원래 style key, 언어, 배경, 나머지 속성과 바깥 XML 바이트를 유지한다. 원래 값과 같은 명령은 source/Undo를 만들지 않는다.
- 하나의 Run에 속한 확정 문자열 치환은 해당 스타일을 보존한다. 실제 선택/커서로 치환 위치를 확인하며 한글/이모지는 Unicode scalar 좌표, 표시 span은 UTF-8 byte 좌표로 구별한다. 연속 입력의 Run affinity와 문서 Undo/Redo의 source-bound affinity를 보존한다.
- IME preedit는 화면 draft에만 있고 Save/dirty/문서 Undo에 들어가지 않는다. Commit만 원문에 반영한다. raw 입력 FIFO는 서식 있는 텍스트의 이동·명령·입력을 순서대로 전달한다. 이 delivery/미지원 경계 거절 정책은 독립 구현이며 원본 WPF 전체 동작의 복원이 아니다.
- 원문이 같은 서식 변경은 양의 format-only proof를 얻은 뒤 기존 계산 source/result 검증과 writer 다음에 COW로 적용한다. 원래 NativeMathml·plot 결과 바이트와 Pending 상태를 유지한다. 글자 내용 변경·새 복제 owner는 일반 context 검증 전에 반영하여 오래된 결과를 보수적으로 Pending 처리하고 payload는 남긴다. 다른 수식/설정 변경이 서식 proof를 이용해 결과를 최신으로 승격할 수 없다.
- 원본 Text의 영역 복사·복제는 문서를 바꾸지 않고 clipboard/candidate에 원문 carrier를 붙인다. 붙여넣기는 실제 목적지 archive의 동일 Text authority가 확인돼야 확정된다. 다음 ID·선택·history 변경 전에 원문/예산을 검사한다. 글자 선택 단위 rich clipboard를 구현했다는 뜻은 아니다.
- 현재 source의 readonly visual overlay를 Open/확정/Undo/Redo 때 갱신하며 같은 문자열의 옛 서식으로 되돌아가지 않는다. 편집 geometry 미지원과 readonly 표시를 분리했다. 가운데 정렬·문단 배경 등은 원래 readonly/인쇄 표현을 유지한다.

## 입력 비용과 실패 원자성

idle에는 원문 ZIP/XAML을 다시 읽거나 선택 속성 목록을 다시 만들지 않는다. 선택 target 캐시 검사를 문단 좌표 순회보다 먼저 한다. 원본 authority와 표시·편집 인덱스는 활성 Text owner만 유지하며 문서 history는 XAML Arc를 공유한다. visual fingerprint는 불변 source별 한 번 계산하고 wire에는 저장하지 않는다.

확정 키의 source 예산은 Rust 문자열/트리 바이트 순회만 사용한다. 기존 source에서 해당 owner의 plain+XAML 비용을 뺀 headroom과 새 비용을 비교한 뒤 runtime/source를 함께 게시한다. 전체 archive 복제·다른 carrier 원문 재해석·결과 projection은 키 입력 검사에 없다. 복사/서식 등 명령은 별도의 borrowed prospective admission으로 실제 authority를 확인한다. 이 경로 구분은 구현 설명이며 실제 LCC 체감 지연 개선 수치가 아니다. 기존 [workspace CPU 측정](0191-lcc-workspace-cpu.md)의 한계도 유지한다.

## 검증 이력

원래 실패 로그를 보존하고 수정 후 별도 로그를 만들었다.

- [첫 export 검사](../../artifacts/rea/rich-text-r206-export-preflight-20261020.log): 35 PASS/1 FAIL. 이미 Pending인 결과 속성까지 다시 써서 quote 바이트가 달라졌다. 기존 PendingKeepPayload가 같은 상태를 다시 쓰지 않도록 고친 [재검사](../../artifacts/rea/rich-text-r206-export-repair-preflight-20261020.log)는 36 PASS다.
- [첫 document 전체 검사](../../artifacts/rea/rich-text-r206-document-preflight-20261020.log): 288 PASS/3 FAIL/7 ignored. 새 visual 테스트 fixture에 명시 weight가 없어 source API의 추론 금지 조건에 걸렸다. fixture에 Normal을 명시한 [재검사](../../artifacts/rea/rich-text-r206-document-repair-preflight-20261020.log)는 291 PASS/7 ignored다. 제품의 미지 속성 추론을 허용하지 않았다.
- [첫 app 전체 검사](../../artifacts/rea/rich-text-r206-app-full-preflight-20261020.log): 734 PASS/1 FAIL/68 ignored. 크기 Enter 뒤 lost-focus가 같은 명령을 다시 내보냈다. 확정 draft 중복 전송을 막은 [리본 재검사](../../artifacts/rea/rich-text-r206-ribbon-repair-preflight-20261020.log)는 6 PASS다.
- [실제 TextEdit 통합](../../artifacts/rea/rich-text-r206-integration-expanded-preflight-20261020.log): 8 PASS. 원본에서 추출한 두 Run과 catalog를 정상 생성한 outer MCDX에 넣은 독립 fixture로 키 입력·alpha 색상·크기 범위·IME Save·Undo·own/native cold 표시·readonly geometry를 확인했다.
- [예산/복사 통합](../../artifacts/rea/rich-text-r206-copy-budget-preflight-20261020.log): 10 PASS/1 ignored. 원본 영역 clipboard·목적지 authority·복제·native 재열기와 전체 8 MiB에서의 source/runtime/history/ID 원자적 실패를 포함한다. [source budget/document 검사](../../artifacts/rea/rich-text-r206-source-budget-preflight-20261020.log)는 298 PASS/7 ignored다.
- [새 artifact probe](../../artifacts/rea/rich-text-r206-artifact-preflight-20261020.log): 실제 own 3개/native 4개를 cold 7회 확인했다. 이 실행은 최종 고정 소스/양 OS 통합 검사를 대체하지 않는다.
- 초기 candidate의 egui ByteIndex/run_ui 적응과 Clippy 수정 로그도 artifacts/rea에 남겼다. 마지막 [Clippy preflight](../../artifacts/rea/rich-text-r206-final-clippy-repair-preflight-20261020.log)는 workspace/all-targets `-D warnings`를 통과했다.

## 남은 범위

D08과 HD038/039/040을 미구현에서 부분 구현으로 갱신했다. HD035의 지원 깊이를 갱신하며 상세555행은 부분156/미구현399, 상위87행은 구현4/부분69/미구현14다. 이 분류는 제품 완성률이 아니다. 새 일반 Text의 rich 서식 생성, 문단 추가/삭제, cross-Run·cross-문단 내용 치환, collapsed caret의 word/spring-loaded formatting, rich 글자 선택 clipboard, 전체 font family/style catalog/underline/정렬 명령, embedded math·Span·hyperlink·모든 WPF 문단 geometry가 남아 있다. 소스 보존과 XSD/벡터 PDF 구조 검사는 원본 Prime의 실제 재열기 또는 픽셀/폰트/줄바꿈 동일성을 증명하지 않는다. 전체 앱 성능과 40% 목표도 완료하지 않았다.

## 최종 고정 검증

`checkpoint_accepted: true`. [고정 검증](../../artifacts/rea/rich-text-integration-20261020-r206/validation.json)은 현재 제품과 archive 일치를 확인했다. 최종 고정 gate/probe/validator 재실행은 없었다. 위 개발 중 실패/수정 기록은 보존했다.

| 항목 | 결과 |
|---|---|
| source archive | 13,824,000 bytes · SHA `c2e771a1c9af14fa009bf43fde9425985b10b3e91567912f28a71525fe770d9b` |
| source manifest | 547 files / product Rust286 · SHA `fb3aafa85c8d70bbbb2bfd1298602ff1c45fc991fd2ea18509e99deff2033864` |
| Windows / peets | 각각 workspace+backend1701 PASS, 실패0, ignored89 |
| format / Clippy / build / reference | 양 OS PASS; Windows release PASS |
| MCDX / standalone XML | 142 / 10 · XSD862parts PASS |
| 벡터 PDF | 70 files · image0 · fitz+pypdf text 검사 PASS |
| 새 rich source flow | own6/native8 · 양 OS 총 cold14회 · 원본 metadata/catalog/resources 유지 |
| 새 PDF 추가 검사 | 6 files · 선택 글자18pt, RGB 양자화 허용오차, alpha200/255 검사 PASS |
| 실행 파일 | [r206](../../artifacts/rea/mathcraft-rich-text-r206-20261020.exe), 25,361,920 bytes · SHA `08dda7ccb35501b13cafa7cef39d14081977ffcf709a42ad624586c696ccc86a` |
| validation | SHA `be568a6b357e35a3111b841465d12711f8d9bb892e5787fd890a4bf27c4dbee1` |

원본 앱을 실행하거나 창을 열지 않았다. PDF 구조/텍스트/색상 검사는 원본 픽셀·전체 font/줄바꿈 동일성 또는 실제 GPU 지연 해소의 증거가 아니다. 후속 [선명도 정적 분석](../../artifacts/rea/text-sharpness-next-20261020.md)은 아직 수정·측정 채택과 구분한다. 40% 목표는 계속 진행 중이다.
