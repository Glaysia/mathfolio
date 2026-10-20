# 수식 입력과 계산 확정의 경계

2026-10-16. 대상은 설치된 Mathcad Prime 12.0.0.1이다. 사용자가 보고한 매 키 입력 재계산과 Enter 확정의 차이를 조사했다. 보존한 REA 관리 어셈블리 증거를 재사용하고 원본 파일 SHA-256과 선택 메서드의 IL SHA를 다시 검증했다. GUI·스크린샷·원본 앱·CLR·COM을 실행하지 않았다. 이 문서는 관찰된 계약이며 Mathcraft의 구현 완료 기록은 아니다.

## 사용자 조작의 계약

설치 도움말은 일반 수식의 **입력 중 편집과 계산 확정을 구분**한다. `Tutorials/gs_tutorial/task2-2_editing_an_equation.html`은 연산자를 교체한 뒤 Enter를 누르면 재계산되고, 괄호·연산자·피적분식을 수정한 뒤 수식 영역 밖을 클릭해도 재계산된다고 설명한다. SHA-256은 `3accf518fdd3cf1ec601e8e71d824e226c86b7784b9338e626e24f543d56c067`이다.

`Tutorials/gs_tutorial/task3-1_defining_and_evaluating_variables.html`은 결과 단위를 지우는 동안 결과가 흐려지고, 영역 밖을 클릭하면 균형을 맞추는 단위가 복구된다고 설명한다. 새 결과 단위 `min`을 입력한 뒤 Enter를 누르면 새 단위에 맞춰 결과를 재계산한다. SHA-256은 `2da34c4aacd9d1ab1a64aa57e7121085f61067408c5d589af3d9048e466b0636`이다. `to_plot_data.html`은 축 식을 입력한 뒤 Enter를 눌러 trace를 표시한다고 설명한다. SHA-256은 `6afe38cbb3251fc35988da568db73ef549b768615e063d136daa4929a70a6876`이다.

따라서 사용자가 수식 숫자를 편집할 때 각 키마다 전체 의존 영역을 재계산하는 동작은 이 일반 수식 입력 계약과 맞지 않는다. 입력 자체는 즉시 조판하되 계산에는 마지막으로 확정한 값을 사용하고, Enter로 영역 편집을 마치거나 다른 영역으로 이동할 때 수정 내용을 계산에 전달해야 한다. 이것은 시간 지연을 둔 debounce와도 다른 경계다.

## REA 정적 근거

| 어셈블리 | SHA-256 | REA Evidence |
| --- | --- | --- |
| `Ptc.Controls.EquationControl.dll` | `0b6fd580eb6cc183f17b5b5e0967226bbafc5e12efc6da809f44cd16416bf9d1` | `ev_389338a9845c7ea83dfd310922316cf92ca1637bdbac6602a01c4690e52b8349` |
| `Ptc.EquationEditor.dll` | `d419032c0e99a62eab2f278e571a338d8c4b9662655af8ab462d482a028c0223` | `ev_419ab24312df266f2e58ff6b91d32e0261a9a2bba9105ba09ebecb7517b43fdf` |
| `Ptc.Controls.Worksheet.dll` | `c1a70ec8007228ff119d4a31b36b9062132be3a10c771459c1dbe3fd0a7baddd` | `ev_ca6ee361e44f5b877e72b99111bddb37dcf24e4dd6163ce516a16f3ad2b2611b` |
| `Ptc.Controls.Whiteboard.dll` | `947ae0dea0c7a7fdbd1c377e4e7f2447dccd5e839111fec0c0789dafffc1cbd9` | `ev_b5601734c03d7bc9573770a236f1d7d0f7c2f9203276c86b4dff938362dc44ab` |

ignored `artifacts/rea/edit-commit-decode.py`는 위 증거의 원본 경로·메서드 본문 위치와 SHA를 확인하고 `edit-commit-cil.json`/`.txt`를 만든다. 원본 구현 본문은 이 문서나 Rust 소스로 복사하지 않는다.

| 메서드 | IL SHA-256 | 관찰 |
| --- | --- | --- |
| `EquationModel.FireExpressionValueChanged`, `0x060001c0` | `b754de85d73fd9ecb81a0dfaadc1e3aff1db28d7ea9dd54eca337ea4d0469e45` | `IsExpressionValueModified`를 true로 바꾸고 식 변경 이벤트·조판 변경을 전달한다. 여기서 RequestEvaluation을 호출하지 않는다. 마지막 보조 메서드 `0x060001c1`도 결과 단위 변경 flag를 처리하며 계산 요청을 보내지 않는다. |
| `EquationControl.get_IsCommitted`, `0x06000047` | `41a0ab9ff78a1c0c187fd8cb83d019d631fac0b8d2c9fa2a0107620579448c7e` | 모델의 `IsExpressionValueModified`가 false인지 반환한다. 입력 수정과 계산 확정이 별도 상태다. |
| `EquationControl.HandleExpressionValueChanged`, `0x0600009b` | `a63904d993e825b4e3e1c1780cac22856fa6fac7773abecb6ad451d6e6b68141` | 현재 수치 서식과 IsCommitted 알림을 갱신한다. 이 handler 자체에는 commit·RequestEvaluation 호출이 없다. |
| `EquationControl.FireRequestEvaluation`, `0x06000153` | `ffbb0972a212fe2e66a8490398d8a31a91e75d3fb68b8d9d1b6662e0d751c6f3` | 수정 flag를 지우고 aggregate를 모델에서 갱신한 뒤 자식·컨트롤 계산 요청을 보낸다. `Commit`은 강제 인자로 이 경로를 호출한다. |
| `WorksheetControlContent.set_ActiveItem`, `0x060005ca` | `7344c28e9412ac839e4076540e830d0064cf67402e16493da7213321702f8693` | 활성 item이 바뀔 때 기존 영역을 비활성화하고 `CommitItemIfNeeded`를 호출한 다음 새 item을 활성화한다. Enter만이 유일한 확정 경계가 아니다. |
| `WorksheetControl.PreSave`, `0x060004b9` | `3b340462e0a22a1ff8cb012655657d0fdf9ac9ac5e910dec5963d2b3171dfb1f` | 일반 저장 시 확정되지 않은 활성 계산 영역·해당 자식을 모아 `FireCommitAndNotifyDependentCalculatables`에 전달한다. 예외 처리 저장에는 early return이 있다. |

Mathboard 내부 영역의 `DeactivateActiveItem` (`0x060003b7`, IL SHA `02b8c32bb29abe15aaff5eefcf2dc5db48755c573a414afdce328b14c1c50dc9`)에도 기존 영역·자식의 IsCommitted 검사와 Commit 호출이 있다. 영역 비활성화의 확정은 평면 워크시트에만 한정된 우연한 연결이 아니다.

## Enter·계산 명령·저장은 구분해야 한다

Enter가 항상 전역 commit인 것은 아니다. `EquationCommands::.cctor` (`0x06000025`, IL SHA `54d4a80893e8a2de79dadeecd117ff166e44afd38b6462e67429de882ce6124a`)는 IL `0x06c2..0x06d1`에서 Enter를 `SplitLinearNaryLine`에 연결한다. 프로그램 줄 삽입·구조적 줄바꿈 등 수식 내부에서 소비하는 Enter를 워크시트 확정으로 가로채면 원본의 작성 방식을 망가뜨린다. 이번 분석은 모든 Enter의 routed-command 우선순위나 모든 프로그램 문맥을 완전히 추적한 것은 아니다. 내부 편집이 Enter를 소비하지 않아 영역을 떠나는 일반 경계를 유지해야 한다.

[0032](0032-calculation-commands-and-saved-results.md)에 기록한 F5의 활성 영역 강제 계산은 `FireCalculateRegionNow`에서 명시 commit을 하고, Ctrl+F5는 `FireRecalculateAll`에서 활성 영역을 commit한다. 수동 계산 모드에서 단순 편집 확정은 값을 새로 계산하는 명령과 다르며, 확정 후에도 이전 결과와 Pending 상태가 함께 남을 수 있다. 자동 계산을 켜는 것과 Stop의 구분은 [0039](0039-calculation-queue-and-cancellation.md)를 따른다.

저장의 PreSave 경로는 수정 내용을 확정하고 종속 영역에 알리는 근거다. 이것만으로 저장이 항상 전체 워크시트 재계산을 완료한 뒤 반환한다고 해석하지 않는다. CalculationController의 `HandleCommitAndNotifyDependentCalculatables` (`0x06000019`, 보존된 `feedback/calc-cil.json`, IL SHA `a5d86d786584bf081aa2e95f6f84e9660de22fadc013d81b463287e50efa27df`)는 worker 중지·동기 callback 전환·commit·callback 모드 복원·worker 재시작 경로를 가진다. 모든 종속 결과의 완료를 보장한다는 주장은 이 정적 증거보다 넓다. 인쇄 역시 원본 도움말에 수동 모드의 이전 값이 출력될 수 있다고 명시되어 있다.

## 적용 범위와 남은 확인

일반 수식 입력에서는 편집 중 AST·커서·Undo와 확정된 계산 입력을 분리한다. 각 키 입력은 화면에 즉시 나타나며, 이전 계산값은 유지한다. 타이핑만으로 전체 문서 MathML 생성·엔진 update·자동 계산 작업 제출을 반복하지 않는다. 확정 시 의미가 바뀌었을 때 한 번 계산에 전달한다. 저장·명시 계산·영역 이동은 각각의 기존 경계를 사용한다.

이 원칙을 slider·checkbox·scriptable control처럼 입력값 변경 때 즉시 계산해야 하는 컨트롤에 일괄 적용해서는 안 된다. 이번 조사에서는 원본의 해당 컨트롤에 `HandelValueIndexChangedAndCommit` 등 별도 경로가 있음을 메타데이터에서 확인했지만 모든 컨트롤 수명 주기는 추적하지 않았다. 창 전체 focus 상실, ribbon focus 이동, 미완성 수식의 복구, Escape 의미, 입력 중 이전 비동기 결과의 표시 순서는 추가 확인이 필요하다. Enter와 영역 밖 클릭에 대한 일반 수식 확정 계약은 도움말과 CIL이 함께 뒷받침한다.
