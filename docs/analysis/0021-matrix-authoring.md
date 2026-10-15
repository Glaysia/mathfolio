# Prime의 행렬 행·열 작성 명령과 독립 구현

2026-10-15. 원본 앱·GUI·스크린샷·CLR·COM 없이 기존 REA 자료를 다시 추적했다. 행렬 입력은 크기만 다시 지정하는 대화상자가 아니라 현재 셀의 위치와 커서 경계를 유지하는 편집 명령이다.

## 원본 근거

대상 `Ptc.EquationEditor.dll` SHA-256 `d419032c0e99a62eab2f278e571a338d8c4b9662655af8ab462d482a028c0223`, MVID `d8627dca-3258-470c-b4d1-c00bbe2f2441`. REA Evidence `ev_419ab24312df266f2e58ff6b91d32e0261a9a2bba9105ba09ebecb7517b43fdf`. 메타데이터는 ignored `artifacts/rea/editor/equation-members.json`, 원문 CIL SHA를 대조한 보조 디코딩은 `equation-il.json`이다. 토큰은 해당 빌드에 한정된다.

| 관찰 | 원본 메서드 | Raw CIL SHA-256 |
| --- | --- | --- |
| 행 삽입은 Direction 인자를 갖는 하나의 명령 | EquationModel.InsertRow `0x06000187` | `78254a506959d810bf84aae4590e88b488761ab50615d5148556efd0f6f88db8` |
| 열 삽입도 Direction 명령 | EquationModel.InsertColumn `0x06000188` | `dd9b25d5129b0341ff36e90799d4e3a0b97e408a0edad9515825d4136a0750d6` |
| 현재 행과 열을 계산하고 삽입 행의 같은 열에 이동 | `b.bf(Direction,bool)` `0x06000239` | `cf2c357e4eb2ba730d89cc2668e0349c38c497c1f0c806b2d55f5e7f5d5d435f` |
| 현재 열의 앞/뒤 삽입, 같은 행에 이동 | `b.bg(Direction)` `0x0600023a` | `c9b665a4ca254c699b9bf7f13a9c971ebf32e6c4e431ffa84568c354b0b9f6f8` |
| 자동 방향은 셀 전체의 왼쪽 경계를 확인 | normal mode `l.aq` `0x060000a1` | `017410cf1579898f6b0fec0c29ffdf2851518220819fe613c550f94e876203f4` |
| 행 삭제와 남은 셀 위치 선택 | `b.b` `0x06000256` | `b71b943423a0c195165c6a59f7bee798156eb74eaa4ddf0c30b0def90c260b08` |
| 열 삭제와 남은 셀 위치 선택 | `b.a` `0x06000257` | `ab634b2ae2503815116f0dfeffbbe0d1ce725e1fe45e529368cb7c9093f3f294` |

`keyboard-shortcuts.txt`의 배포 Keyboard Shortcuts PDF page21은 Shift+Space가 커서 위치에 따라 왼쪽/오른쪽 열을, Shift+Enter가 위/아래 행을 삽입한다고 명시한다. 항상 마지막 행에 추가하는 동작이 아니다.

행 삽입 `0x06000239` IL_004e–0063은 기본 offset1을 두고 명시 Up 또는 Indeterminate에서 mode의 `aq`가 true일 때 offset0으로 바꾼다. IL_009a–00a9는 현재 셀의 RowOf+offset을 삽입 위치로 쓴다. IL_00ab–00be는 일반 InsertRow 경로에서 현재 열을 보존한다. 열 삽입 `0x0600023a` IL_0061–0075는 같은 방식으로 Left/Indeterminate를 구분한다. 식별한 Direction 값은 기존 Core 조사대로 Left0, Right1, Up2, Down3, Indeterminate4다.

normal mode `l.aq`는 현재 노드가 container root의 첫 경계에 있는지(`bq.b`→`bq.a`, `0x06000604/605`), container root의 부모가 해당 행렬인지, WhatSideOfTheNodeIsTheCaret가 Left인지 확인한다. 분모나 합의 오른쪽 항 내부에서 단지 문자 offset0이라고 새 행/열을 앞에 삽입하는 것은 이 근거와 다르다.

삭제 진입점 EquationModel.DeleteRow/DeleteColumn `0x06000189/18a`는 undo 명령을 거쳐 현재 모드에서 가장 가까운 행렬과 그 자식 셀의 row/column을 찾는다. 단일 셀 문맥의 행 삭제는 `0x0600023b`, 열 삭제는 `0x0600023c`다. `0x06000256`은 제거할 행을 건너뛰어 살아남는 셀 참조들을 새 row-major 배열에 복사한다. 최종 행을 제거해 0행이 되면 1×1로 바꾸고 활성 column0을 선택한다(IL173–184). 열 삭제도 대칭 처리한다(IL167–178). 새 크기에 맞는 기본 노드 보충은 `r.a 0x060002f1`→`0x060002f3`→`0x060002fa`가 담당한다. 행·열 수가 남아 있을 때 선택 좌표는 제거된 index를 쓰되 범위를 벗어나면 마지막 index로 줄인다.

원본의 다중 셀 typeover, result matrix의 viewport 및 scriptable control 처리도 별도 분기로 존재한다. 이번 구현에서 그 전체를 지원한다고 주장하지 않는다. 원본은 새 matrix wrapper를 생성하는 경로를 쓰지만 Mathcraft의 노드 ID는 독립 편집 모델의 ID이므로 기존 matrix ID까지 유지하도록 구현했다.

## 구현 API

`crates/mathcraft-editor/src/lib.rs`:

```rust
pub fn insert_matrix_row(&mut self, direction: Direction) -> bool;
pub fn insert_matrix_column(&mut self, direction: Direction) -> bool;
pub fn delete_matrix_row(&mut self) -> bool;
pub fn delete_matrix_column(&mut self) -> bool;
```

행은 Up/Down/Indeterminate, 열은 Left/Right/Indeterminate를 받는다. 잘못된 방향은 무동작이다. 키보드는 Indeterminate를 전달하고 리본의 명시 앞/뒤 버튼은 해당 방향을 전달한다. 모든 명령은 현재 셀을 포함하는 가장 안쪽 행렬에 적용하며, 행렬 전체 선택은 모호한 문맥으로 거절한다. 부분식·일반 텍스트 선택을 담은 셀에서도 해당 행·열을 찾는다.

자동 앞 삽입은 셀의 첫 leaf에서 왼쪽 caret 또는 collapsed UTF-16 range0일 때, 또는 셀 전체의 왼쪽 scope일 때다. 셀 내부의 다른 leaf의 offset0은 뒤 삽입이다. 새 placeholder의 Indeterminate caret은 뒤 삽입을 선택한다. 이는 빈 셀의 완전한 원본 NavigatorState를 아직 표현하지 않는 현재 모델의 명시적 정책이다.

살아남는 셀·부분식·형제·matrix의 ID와 row-major 순서를 유지한다. 삽입 후 새 row의 같은 column 또는 새 column의 같은 row로 이동한다. 삭제 후 같은 index의 다음 생존 행/열, 끝이면 이전 행/열의 첫 leaf 왼쪽으로 이동한다. 제거된 부분식은 자식까지 tombstone 처리한다. 마지막 축을 지우면 matrix는 1×1 Placeholder로 남는다. 이미 빈 1×1을 다시 지우는 것은 무동작으로 처리한다. 원본이 이 반복 명령까지 undo 기록하는지는 검증하지 않았다.

실제 변경은 명령당 하나의 checkpoint다. Undo는 삭제된 셀·정확한 caret/value range/selection을 복원하며 Redo는 같은 ID를 복원한다. `edit_group`에 포함하면 기존 그룹의 원자성을 따른다. 실패한 방향·문맥·자원 한도 검사는 checkpoint 전에 끝내므로 redo를 지우지 않는다.

`MAX_MATRIX_CELLS=4096`과 전체 live node `MAX_TREE_NODES`가 삽입 한도다. 원본 제품의 행렬 한도라는 주장이 아니다. 기존 append API도 같은 삽입 helper를 사용해 한도와 undo를 일관되게 했다.

## 검증

editor crate 전체 테스트25개 통과. 새 회귀 테스트6개는 위/아래 및 좌/우 삽입, row-major 안정 ID, 새 셀의 행/열 유지, 셀 전체 경계 판별, 삭제 시 next/previous 선택, 마지막 축의1×1 초기화, 중첩 행렬·형제 보존, 전체 행렬 선택 거절, 한도 초과 및 실패 후 redo 보존, undo/redo의 caret 복원을 검사한다. `cargo fmt -p mathcraft-editor`, `cargo clippy -p mathcraft-editor --all-targets --locked -- -D warnings`도 통과했다. 실제 원본 GUI 차등 검증은 포함하지 않는다.
