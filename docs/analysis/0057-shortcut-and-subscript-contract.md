# 원본 단축키와 이름 아래첨자·비교식 편집

2026-10-16. 원본 Mathcad Prime 앱·GUI·스크린샷·CLR·COM 실행 없이 설치된 문서와 REA 관리형 metadata/IL을 대조했다. 원본 단축키의 의미를 편집 AST 및 계산용 MathML까지 연결하는 조사와 구현이다. 모든 원본 단축키를 구현했다는 보고는 아니다.

## 단축키와 저장 의미

원본 설치 `Doc/en-US/keyboard_shortcuts.pdf` SHA-256 `db4cdf1f913cfad20075b3f6354085183a22bde588900709c12ca06c065b3a4f`의 텍스트는 기존 ignored `artifacts/rea/editor/keyboard-shortcuts.txt`를 재사용하고 원본 SHA를 재확인했다. 문서는 12.0.0.0이고 설치 바이너리는 12.0.0.1이므로 문서 계약과 실행 관찰을 구분한다.

| 키 | 원본 계약 | 독립 편집기의 연결 |
| --- | --- | --- |
| Ctrl+- | 이름/텍스트의 일반 글자 ↔ literal subscript, PDF p7 | 수식 이름의 suffix 편집·커서 전환. 확대/축소 명령이 아니다. |
| Ctrl++ | 텍스트 region의 superscript, p7 | 전역 zoom 충돌 제거 대상. 수식 지수 입력은 `^`; 텍스트 superscript는 이번 구현 범위 밖이다. |
| Ctrl+= | Boolean Equal To, p13 | `BinaryOp::Equal` → `ml:equal`. 일반 `=` 평가 명령과 구분한다. |
| Ctrl+9 / Ctrl+0 | ≤ / ≥, p13 | 각각 `ml:lessOrEqual`, `ml:greaterOrEqual` |
| `<`, `>`, `<=`, `>=`, `<>` | 비교 연산, p13 | 비교 AST. 두 번째 키가 pending RHS에서 들어오면 기존 연산자만 교체한다. |
| Ctrl+Shift+1/2/5/7 | NOT/OR/XOR/AND, p14의 영어 키보드 `!/@/%/&` | 각 논리 AST와 원본 MathML operator에 연결한다. |
| Ctrl+Shift+Z | ∞, p8 | 문자 삽입. literal suffix 안에서는 이름 일부이며 계산 연산자로 바꾸지 않는다. |
| Ctrl+M, `[` | matrix, p17 | Ctrl+M으로 1×1 matrix 생성. 기존 overloaded `[`의 matrix/index 문맥 분기를 유지한다. |
| Ctrl+Left/Right | 현재 term 경계와 다음 term으로 이동 | 반복 명령이 같은 atom 끝에서 멈추던 경로를 보완했다. 전체 원본 navigator를 복원했다는 뜻은 아니다. |

설치 도움말 `Help/en/PTC_Mathcad_Help/using_literal_subscripts_for_name_definitions.html`, SHA `dbb5635daeb5199c8ffb985c12700231f7070d2582ea9dc03c6da2ef1826fa05`는 literal subscript가 변수/함수 이름의 일부이며 배열 원소를 지정하는 index와 다른 기능이라고 명시한다. Math 탭 Subscript와 Ctrl+-가 같은 명명 작업을 제공한다. 원본 XAML `Span`/`Subscript`를 가진 `ml:id`는 기존 bridge의 `NativeIdentifier.subscript`로 보존되며 `ml:indexer`로 바꾸지 않는다.

## REA 정적 증거

기존 retained evidence와 설치 파일의 SHA, 선택한 raw IL SHA를 다시 확인했다. ignored `artifacts/rea/shortcut-contract-decode.py`는 dncil로 바이트만 읽으며 원본 메서드를 실행하지 않는다. 결과는 `shortcut-contract-cil.json`에 보존한다.

- `Ptc.EquationEditor.dll`: SHA `d419032c0e99a62eab2f278e571a338d8c4b9662655af8ab462d482a028c0223`, evidence `ev_419ab24312df266f2e58ff6b91d32e0261a9a2bba9105ba09ebecb7517b43fdf`.
- `Ptc.EquationControl.dll`: SHA `0b6fd580eb6cc183f17b5b5e0967226bbafc5e12efc6da809f44cd16416bf9d1`, evidence `ev_389338a9845c7ea83dfd310922316cf92ca1637bdbac6602a01c4690e52b8349`.
- `EquationControl.OnToggleSubscript`, token `0x060001bd`, IL SHA `49519e15985a17be7bc57d0e59117fd0e53b0365ff6e475aa00f5620ef1ff217`, delegates to view `FireToggleSubscript`.
- `VisualMathRegionNode.FireToggleSubscript`, `0x06000a45`, SHA `c72909ea5a73b30042062297b526f158f3b5a40851b107d78d87e3d9699cab27`, invokes its ToggleSubscript action. `EquationModel.ToggleSubscript`, `0x06000165`, SHA `2c8331cb71c668c377bc0b6c7c4dda768d537634365f200951ce6d44fe1bba6b`, delegates to an active implementation. The base `b.m`, `0x0600024e`, is empty; it does **not** prove every concrete mode's insertion behavior.
- `VisualMathRegionNode.OnKeyDown`, `0x06000a09`, SHA `ff5bfd9dbd3877bce720277c70d0698916d169f73588ecd19a81c1cef998e45f`, first routes through its active gesture manager. This supports interpreting a key in the focused math context instead of always treating it as a global viewport command.
- Descriptor initializer `0x06000bbb`, SHA `90ff4afc15ec4d75068d5d2c0eab08be91548ff38974f8de5b54e02a25a6376a`, supplies the comparison/logic XML operator names and binding powers. Relevant constructor IL offsets are greater 4747, less 4921, greater-or-equal 5095, less-or-equal 5281, equal 5467, not-equal 5653, AND 5839, OR 6020, XOR 6201, NOT 9559. Comparison precedence is 80, AND 72, XOR 71, OR 70 and NOT 150. XOR's actual character constant is 8853 (`⊕`), observed at initializer IL 6210 and 6342. Glyph choice is not guessed from another product.

Boolean equality의 실제 character constant는 IL 5476/5600에서 65309(`＝`, U+FF1D)다. 일반 평가 등호와 같은 ASCII `=`로 추정하지 않고 이 넓은 glyph를 원본 구조에 사용한다.

## 원인과 변경

기존 `Editor::toggle_subscript`는 caret flag만 전환했고 formula의 텍스트 입력·삭제·hit test는 그 flag를 사용하지 않았다. 가져온 아래첨자는 그려졌지만 직접 편집할 수 없고 클릭 영역도 base 이름 부분에만 있었다. 전역 zoom binding은 Ctrl+-를 먼저 소비했다.

새 `toggle_literal_subscript() -> bool`은 별도의 literal suffix 필드를 열거나 그 필드와 base 이름 사이로 커서를 이동한다. 명시 label·contextual 여부·원래 이름을 보존한다. `place_subscript_selection`은 UTF-16 surrogate 경계를 검사하고 suffix 문자 선택을 지원한다. 처음 빈 suffix를 만들 때만 문서가 변하며 기존 필드 사이의 이동은 계산 변경이 아니다. 기존 저수준 `toggle_subscript()`는 호출 계약을 깨지 않기 위해 유지하고 UI는 새 API를 사용한다.

formula 입력은 suffix 모드에서 `-`, `=`, `/` 등을 이름 문자로 처리한다. 이를 빼기·계산·분수 AST로 오인하지 않는다. 삭제, IME 주변 문자 삭제, 선택 교체·복사, undo와 좌우 문자 이동도 동일한 suffix 필드를 사용한다. suffix 끝과 base 경계를 넘는 커서 동작 및 별도 suffix 클릭 hit와 낮은 caret 좌표를 추가했다. imported suffix와 표시 단위 뒤의 suffix도 원본 메타데이터를 유지한다.

비교·논리 연산은 숫자 엔진에 이미 있던 기능을 editor AST에서 만들 수 없던 연결 누락을 고친 것이다. `BinaryOp`에 6개 비교와 AND/OR/XOR, unary `LogicalNot`을 추가하고 원본 XML operator의 parse/serialize를 구현했다. 키 입력과 MathML을 경유하여 기존 unit-aware boolean evaluator로 전달한다. Ctrl+=는 `equal` 구조만 삽입하고 자동 계산 요청을 보내지 않으며, 일반 `=`는 원래처럼 명시 평가 명령이다. `<=`/`>=`의 두 번째 `=` 역시 비교 연산 완성일 뿐 평가 요청이 아니다.

## 검증과 아직 남은 경계

headless egui 프레임을 통한 실제 key event 테스트는 Ctrl+-·Ctrl+Shift+Z·Ctrl+M·Ctrl+=·Ctrl+9·Ctrl+0·논리 NOT을 확인한다. formula 테스트는 literal 문자와 배열 인덱스의 구분, suffix 선택·Unicode·undo, suffix 클릭/caret 좌표, 비교/논리의 원본 MathML 왕복 및 물리 단위를 가진 비교를 검증한다. `1+2<4*2=`, `2m<3m=`, `2m<3s=` 오류, AND/OR/XOR/NOT, 두 키 비교와 평가 `=`의 구분을 포함한다. 반복 Ctrl+arrow가 literal suffix 끝에서 맴돌지 않고 다음 operand로 넘어가는 경계도 검증했다.

Windows dev focused formula 검사 `cargo test -p mathcraft-app --bin mathcraft --locked formula::tests`는 51개 통과, 기존 benchmark 1개 ignored다. editor+compat 전체 검사도 통과했으며 기존 compat 성능 benchmark 1개는 ignored였다. clippy `--all-targets -- -D warnings`도 editor/compat/app에서 검증한다. 로그는 ignored `shortcut-formula-tests.log`, `shortcut-editor-compat-tests.log`, `shortcut-app-clippy.log`, `shortcut-compat-clippy.log`에 보존한다.

추가로 전체 앱 키 라우팅에서 같은 종류의 충돌을 발견했다. egui 0.36.2의 `consume_key`는 `matches_logically`를 써서 추가 Shift/Alt를 무시하므로 Ctrl+Z 전역 undo가 Ctrl+Shift+Z 기호 입력을 먼저 소비하며, Ctrl+2 plot 삽입이 Ctrl+Shift+2 OR를 소비한다. 독립 widget의 성공을 전체 앱 성공으로 확대하지 않고 전역 라우팅도 수정했다.

새 `worksheet_ui/original_shortcuts.rs::consume_exact_key`는 각 key event의 `Modifiers::matches_exact`를 검사한다. 기존 전역 편집·파일·document-history·영역 clipboard/delete 단축키에 이를 적용하고 Ctrl+Shift+Z의 redo alias를 제거했다. 원본 redo Ctrl+Y는 유지한다. F1과 Ctrl+F1, F5와 Ctrl+F5도 정확한 modifier 조합으로 분리한다. 배정하지 않은 추가 Shift/Alt 변형을 임의로 다른 전역 명령으로 실행하지 않는다. 새 `original_shortcuts` headless 검사는 실제 global-editing/history 함수가 먼저 처리한 이벤트를 page의 formula widget으로 넘겨 아래첨자·∞·OR가 살아 있는지 확인하며, F1 계열과 F5 계산 범위도 검사한다. widget 내부 단독 성공과 전체 앱 키 라우팅 성공을 별도로 검증한다.

`cargo test -p mathcraft-app --bin mathcraft --locked original_shortcuts::tests`의 통합 검사 3개가 통과했다. Ctrl+- 뒤 zoom이 그대로이고 이름 suffix가 열리는지, 실제 document history를 가진 상태에서 Ctrl+Shift+Z가 undo 대신 `dc∞`를 만드는지, Ctrl+Shift+2가 plot region을 추가하지 않고 OR AST를 만드는지 확인했다. F1 도움말과 Ctrl+F1 리본 전환은 서로 영향을 주지 않으며, F5는 visible prefix만 계산하고 Ctrl+F5는 이후 페이지도 계산한다. 추가 Shift/Alt F1 및 Shift+F5를 잘못 실행하지 않는 경계도 포함한다. 로그는 ignored `artifacts/rea/original-shortcuts-global-tests.log`에 있다.

원본이 지원하는 이름 중간의 여러 rich-text run(예: H₂O), 여러 개의 literal subscript 구간을 하나의 이름에 교대로 두는 기능은 현재 `name + suffix` 모델에서 표현하지 못한다. 선택 범위가 base와 suffix를 가로지르는 부분 문자열 선택, 원본의 모든 키보드 언어별 변형, 텍스트 region의 superscript, 모든 navigator 경계도 아직 완전 복원하지 않았다. ∞의 이름/기호 삽입과 무한구간 수치 계산은 별개이며 이번 변경으로 무한구간 계산을 구현했다고 주장하지 않는다.
