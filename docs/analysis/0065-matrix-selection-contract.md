# 일반 행렬의 직사각형 선택과 편집 명령

2026-10-16. 일반 수식 행렬의 다중 셀 선택을 SpecTable의 규칙과 분리해 조사하고 편집 core에 연결했다. 원본 앱·GUI·스크린샷·CLR·COM 실행 없이 설치 도움말과 REA metadata/raw IL을 사용했다. 실행 검증은 독립 Rust 편집기의 headless 테스트다. formula의 포인터·clipboard 연결과 전체 앱 검증은 [0066](0066-matrix-selection-application.md)의 별도 범위다.

## 사용자 도움말

설치 `Help/en/PTC_Mathcad_Help/to_edit_arrays_and_tables.html`, SHA-256 `85cc675cc698b30fc4fd20655032f63743cbf94bc196323c148a6a3ac311beb0`, 12331 bytes는 선택한 행·열을 Delete/Backspace로 지우며 Clear Cells는 셀 내용만 비우고 행렬 크기를 유지한다고 설명한다. 선택 직사각형의 내부/전체 행/전체 열을 구분하는 자세한 조건은 아래의 실제 명령 분기에서 확인했다.

`keyboard_shortcuts_for_editing_arrays_and_tables.html`, SHA `077a67fee05e6ff532ec7a7176ef1a73fb91dbed2a2a53e33eab8d9a9ec54783`, 17194 bytes는 Shift+Enter/Shift+Space의 행·열 삽입 위치, Tab의 row-major 이동 및 마지막 셀에서 새 행 생성을 설명한다. 이 문서에 명시되지 않은 Shift+Arrow 직사각형 확장은 추가하지 않았다.

## 원본 정적 증거와 mode 구분

새 `inspect_managed_members` evidence `ev_f8a6057c3330888f30692f505a3474339761bcc37866eecad10b9a7d88bfd660`는 설치 `Ptc.EquationEditor.dll`, SHA `d419032c0e99a62eab2f278e571a338d8c4b9662655af8ab462d482a028c0223`, 645616 bytes에 묶여 있다. 선택 type들의 coverage는 complete, unmatched name과 issues는 없다. 기존 retained evidence와도 같은 artifact/IL SHA를 재확인했다.

난독화된 타입 이름은 중복되므로 문자열 이름만으로 연결하지 않았다. `EquationModel.ab` token `0x060000d4`, raw IL SHA `84d01e39bba519646863f429e7f2d2ad08968ac3060017c84b1d48879b7594e2`의 생성/field 할당에서 mode들을 구분했다.

- field `l`은 TypeDef `0x02000014`의 `l`, 일반 편집 mode다.
- field `m`은 TypeDef `0x02000020`의 `m`, scoped mode다.
- field `n`은 TypeDef `0x02000022`의 `n`, `TypeoverState`를 사용하는 mode다. 세 타입은 base `b`, TypeDef `0x02000016`을 상속한다.
- 이 조사에서 일반 행렬 선택의 핵심은 별도 이름의 `MatrixSelectionMode` 클래스가 아니라 `n`과 `TypeoverState.AreMatrixCellsInTypeover`였다. 이 predicate는 선택 endpoint의 공통 조상이 MATRIX 또는 ScriptableControl인지 확인한다. ScriptableControl은 이번 독립 구현 범위 밖이다.

추가로 동일 SHA의 metadata MethodImpl table 137행을 정적으로 읽어 위 4개 TypeDef에 explicit MethodImpl 행이 없음을 확인했다. 따라서 `n`에 이름/virtual signature가 없는 Paste override가 숨은 explicit override로 대체되어 있다고 추정하지 않는다. 원본 코드 로딩 없이 바이트 metadata만 읽었다.

ignored `artifacts/rea/matrix-selection-members.json`에는 fresh evidence를 보존했다. `matrix-selection-decode.py`는 artifact SHA와 메서드 raw IL SHA를 검증한 뒤 dncil로 instruction들을 읽는다. `matrix-selection-contract-cil.json`/`.txt`에 offset별 분기와 caller token을 남겼고 `matrix-selection-evidence-identities.json`에는 도움말 SHA, type 좌표, 메서드 SHA만 모았다. 원본 소스를 독립 구현으로 복사하지 않았다.

## 명령별 실제 분기

| 동작 | caller / concrete 메서드 | 정적 관찰 |
| --- | --- | --- |
| 포인터 선택 | `EquationModel.SelectTo` `0x06000150` → mode `al`; 편집 mode `l.al` `0x0600009b` | endpoint의 공통 조상이 MATRIX일 때 별도의 셀 선택 경로로 들어간다. 같은 atom 내부 선택과 matrix 셀 사이 선택을 구분한다. 결과 scale 및 symbolic-result branch에는 추가 guard가 있다. |
| Copy | `EquationModel.Copy` `0x06000147` → `n.a6` `0x060002d5` | corner indices로 inclusive 직사각형을 구하고 각 cell tree를 row-major로 Clone하여 해당 크기의 새 Matrix를 clipboard S-expression으로 만든다. |
| Cut | `EquationModel.Cut` `0x0600014c` → `n.a7` `0x060002d6` | Copy를 먼저 호출한 뒤 일반 삭제 명령을 호출한다. 별도의 spreadsheet shift-cells 정책이 아니다. |
| Delete/Backspace | `n.j` `0x060002b9` → `n.b` `0x060002bb` | 직사각형 경계에 따라 아래 4가지로 나뉜다. 일반 수식 삭제와 같은 식으로 배열을 통째로 placeholder로 바꾸지 않는다. |
| Delete Row | `EquationModel.DeleteRow` `0x06000189` → 공통 dispatch → `n.bh` `0x060002ce` | 선택 직사각형이 걸친 모든 행 번호를 모아 행 삭제 helper에 넘긴다. 직사각형이 행 전체 폭을 채우지 않아도 해당 행들이 삭제된다. |
| Delete Column | `EquationModel.DeleteColumn` `0x0600018a` → `n.bi` `0x060002cf` | 선택이 걸친 모든 열 번호를 모아 열 삭제 helper에 넘긴다. |
| Clear Cells | `EquationModel.ClearCells` `0x0600018f` → `n.bj` `0x060002d0` | 직사각형의 모든 cell index를 모아 내용만 지운다. shape 삭제를 하지 않는다. 일반 편집 mode의 active cell clear 경로도 별도로 있다. |
| 행·열 삽입 | `EquationModel.InsertRow/Column` `0x06000187/188` → `n.bf/bg` `0x060002cc/2cd` | range mode를 해제한 후 false를 반환한다. 이 mode에서 선택된 행 수만큼 삽입하거나 범위를 복제하지 않는다. 일반 active-cell 삽입은 기존 명령대로 유지한다. |
| 범위 Paste | `EquationModel.Paste` `0x0600014f` → 현재 mode의 `aw` | `n`에는 `aw` override가 없으며 base `b.aw` `0x0600024b`는 false를 반환한다. 이 경로를 spreadsheet 모양 맞춤·반복 채움·확장 붙여넣기로 바꾸지 않는다. |
| 범위의 일반 식 입력 | `n.u/v/x` `0x060002bd/2be/2c0` | `AreMatrixCellsInTypeover`이면 false를 반환한다. 선택 범위를 새 수식으로 채우거나 matrix 전체를 연산자로 감싸지 않는다. |

`n.b`의 Delete/Cut 분기 순서는 다음과 같다.

1. 전체 행렬을 덮는 선택: `n.a` `0x060002bc`가 1×1 placeholder Matrix로 바꾼다.
2. 전체 너비를 덮는 선택: 선택한 행들을 삭제한다.
3. 전체 높이를 덮는 선택: 선택한 열들을 삭제한다.
4. 그 밖의 내부 직사각형: 선택 셀들의 내용만 지우고 크기를 유지한다.

두 corner의 row-major index를 단순 선형 구간으로 펼치지 않는다. 예를 들어 4열에서 index 5와 10의 범위는 네 칸 5/6/9/10이다. Delete와 Clear Cells가 항상 같은 명령은 아니며, Delete Row/Column은 일반 Delete의 전체 폭/높이 guard와도 다르다.

주요 raw IL SHA:

| token | SHA-256 |
| --- | --- |
| `0x0600009b` | `5e46d2e03775224c632f97411c3e31009d4e9a8ee420cde294a9d11ddb45435a` |
| `0x060002bb` | `f8bc94c268d3a21e97c4e4a5ab589a82907ce952511228b519566c391be00cfc` |
| `0x060002bc` | `390da99466c29bfec75424e9b66f6def3e32b1ad565e8ad695086aa8720b96df` |
| `0x060002ce` | `9335ff01ee15506410ead1153a5627f106cf9c6b9cd29d77d2b5bd8fa9e47c31` |
| `0x060002cf` | `d097d880688104431834fc91ea1e8b078b14a26073988a1230c15221b37f68e3` |
| `0x060002d0` | `3a899c0e03674bb721c022e23d9ff1ffc418d55a4044f424b32ea454c393c59f` |
| `0x060002d5` | `1239a5639270b7bde5cf57d0a148bef69a60ffc9685d8eebe79021da0b74fc70` |
| `0x060002d6` | `ddfac89a311ce09c42c35638b840c2b19265afc37f11238bdd476c03c793b3a0` |
| `0x0600024b` | `84a0936ff1fba6d2811d094757231685673447aed2170738872305076026a316` |
| `0x060012bb` | `fbe50d8aae610829aecdee8160ef0e80f3a120665a098c1b6b18e0dc316d45de` |

Shift+Left/Right도 별도로 추적했다. `EquationModel.SelectLeft/Right` `0x0600015f/160`는 현재 mode의 `ah/ai`를 호출한다. 편집 mode `l.ah/ai` `0x06000096/97`는 ValueRange가 있을 때 문자 offset을 조절하며 없으면 반환한다. range mode `n`은 이 메서드들을 override하지 않고 base `b.ah/ai` `0x06000225/226`는 빈 메서드다. `SelectToStart/End`의 `ak/aj`도 같은 range-mode 상속 경계다. 따라서 다중 셀 선택을 위해 Shift+Arrow에 새 의미를 부여하지 않는다. Shift+Up/Down의 실제 key caller와 원본 hidden-viewport 선택은 이번 조사에서 완전히 복원하지 않았다.

## 독립 편집 core와 API

`Selection::MatrixCells(MatrixCellSelection { matrix, anchor, focus })`를 추가했다. corner는 Matrix의 직접 children에 대한 inclusive row-major offset이다. `select_matrix_cells`는 node/offset을 먼저 검증하며 선택 자체는 arena revision·문서 undo/redo를 변경하지 않는다. `matrix_selection`과 `bounds(cols)`가 유효한 범위를 전달한다. 기존 PortableTree와 MathML의 저장 형태는 바뀌지 않는다.

`copy_matrix_selection()`은 완전한 cell AST와 label/contextual/literal suffix metadata를 가진 작은 Matrix tree를 반환한다. OS clipboard 쓰기는 formula 계층이 담당한다. `delete_matrix_selection(backwards)`는 위 원본의 4분기를 구현하며 Delete/Backspace에 같은 shape 규칙을 쓴다. `clear_matrix_cells()`는 선택 범위 또는 active cell을 비우고 크기를 유지한다. 행·열 리본 삭제는 선택이 걸친 축 전체를 삭제한다. 범위 상태의 행·열 삽입은 문서를 변경하지 않고 선택 mode만 해제한다.

`paste_matrix_selection()`은 유효 tree를 검사한 뒤 false를 반환한다. `replace_selection`·`set_atom`·수식 wrapping·literal subscript·matrix 삽입도 MatrixCells 상태에서는 거절하여 core 직접 호출이 범위 전체를 뜻밖의 새 식으로 감싸지 않게 했다. Program 내부 행렬 선택에서 program line 삽입으로 잘못 빠져나가던 교차 경로도 no-op으로 막았다. 사용하지 않는 방향별 rectangle 확장 API는 제거했다.

변경 명령은 한 번 checkpoint하고 완료 후 선택을 해제하며 Undo는 원래 tree와 범위 선택을 함께 복원한다. 살아남은 node ID와 outer sibling, 중첩 matrix 경계를 보존한다. 이는 기존 독립 arena/history 구조에 맞춘 구현 정책이며 원본의 내부 node ID나 undo data 구조까지 같다는 뜻은 아니다.

## 검증과 경계

새 `mathcraft-editor/src/matrix_selection.rs`의 9개 회귀는 역방향 corner 정규화·직사각형 Copy·라벨 보존, Clear 크기/ID 유지와 단일 Undo/Redo, Delete/Backspace의 4가지 shape 분기, 리본 행·열 범위 삭제, 선택 중 삽입의 no-op과 redo 보존, paste/수식 명령 거절, 중첩 matrix의 outer sibling 보존, 잘못된 좌표 및 이미 빈 셀의 무변경, Program 내부 선택의 잘못된 줄 삽입 차단을 검사한다.

Windows dev `cargo test -p mathcraft-editor --locked --target-dir target/compat-spec-tests` 전체 43개와 `cargo clippy -p mathcraft-editor --all-targets --locked --target-dir target/compat-spec-tests -- -D warnings`가 통과했다. 로그는 ignored `matrix-selection-editor-tests.log`, `matrix-selection-editor-clippy.log`에 있다. 실제 앱 pointer/clipboard와 양OS 최종 검사는 상위 통합 범위다.

이 변경은 일반 Matrix의 rectangular cell typeover를 지원한다. 원본의 모든 scoped operand 부분 선택, result matrix의 viewport/hidden cell, ScriptableControl 선택, rich-text clipboard 형식, 외부 spreadsheet와의 paste 변환은 복원하지 않았다. 전체 Matrix를 식 자체로 선택한 `Selection::Subtree`와 내부 셀들을 선택한 `MatrixCells`는 별개다. SpecTable은 별도 모델과 명령 경로를 유지한다.
