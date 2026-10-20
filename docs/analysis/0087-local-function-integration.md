# 프로그램 지역 함수와 작성·확정·저장 경계

2026-10-19. 원본의 LOCAL_ASSIGNMENT whitelist·APPLY coercion·native writer 및 실제 도움말 MathML 예제를 [0085](0085-local-function-authoring-contract.md)·[0086](0086-local-function-calculation-contract.md)에 연결한다. GUI·스크린샷·원본 프로세스 없이 REA 정적 증거와 자체 CPU 검사로 검증한다.

## 실제 작성 경로

기존 프로그램의 직접 statement에서 `f(x)`를 입력하고 원본 단축키 `{`를 누르면 호출을 Function LHS로 바꾸고 `←` 오른쪽의 입력칸으로 이동한다. 같은 command는 이름·named Index/Row/Column을 LHS에 보존한다. 기존 helper는 현재 operand를 RHS에 넣어 이 작성 흐름을 만들지 못했다. Program 밖·statement 내부·표의 expression-only 문맥·잘못된 target·행렬 범위에서는 정의를 억지로 만들지 않는다. 빈 statement에는 양쪽 입력칸을 만들고 LHS부터 채운다.

formula의 일반 텍스트 경로에 `{`를 연결했고, literal 아래첨자 안에서는 기존 이름 문자로 보존한다. headless egui Text 이벤트가 실제 focused formula handler를 거치는지 검사한다. `f(x){x^2`를 한 글자씩 입력하고 프로그램 새 줄에서 `f(3)`을 작성한 뒤 native MathML 왕복과 worksheet label 처리를 거쳐 9인지 확인한다. `{` 하나의 Undo/Redo는 원래 Call까지 복원한다. Enter가 프로그램 내부에서 줄을 삽입하는 기존 동작은 유지한다.

가져온 함수만 표시하는 것으로 이 경로를 대신하지 않는다. 미완성 parameter는 편집 초안으로 유지하지만 native writer·계산은 완성된 이름 parameter를 요구한다. 함수값 alias·함수 배열·고차 함수 전달과 원본 전체 프로그램 편집은 지원 완료가 아니다.

## 확정·이력 회귀

프로그램 함수 body의 지수를 2에서 3으로 바꾸면 초안 동안 기존 `f(3)=9`를 유지한다. 전체 수식 경계에서 Enter로 확정하면 27을 계산하고 Undo/Redo도 계산 결과를 복원한다. 프로그램 내부 Enter와 수식 경계의 확정을 구분한다.

리본의 오래된 focus 요청이 남아 있어도 이력의 소유자는 실제로 수정한 selected math region이어야 한다. 이전 `input_snapshot`은 `editing_region/focus_new`를 우선해 다른 영역의 문서 snapshot과 현재 editor snapshot을 섞을 수 있었다. 새 회귀에서 이 실패를 먼저 재현했고, math command용 snapshot을 selected target에 맞췄다. Define·Evaluate·InsertSymbol·Factorial 각각의 Undo/Redo가 문서와 editor를 함께 되돌리는지 확인한다. 실제 typing의 active-focus snapshot과 새 영역 생성의 전체 문서 이력은 유지한다.

## native·자체 저장·PDF 경로

`local_function_save_native_export_reopen_probe`는 여섯 영역에서 다섯 질의를 검사한다. 제곱 함수 9, 두 parameter 합 7, 프로그램 body를 가진 함수 8, worksheet 함수의 외부 parameter를 읽는 지역 helper 8, 물리 입력을 두 배로 하는 함수 6 V다. 자체 저장 문서를 열 때 계산 없이 표시 다섯 개를 복원하고, MCDX를 다시 열면 전부 Math로 편집하며 재계산 오류가 없어야 한다. 함수와 formal은 `localDefine/function/boundVars`로 보존하고 별도 native 태그를 발명하지 않는다.

이 probe는 자체 importer/exporter의 회귀이며 원본 프로그램에서 새 MCDX를 열었다는 뜻이 아니다. 설치 XSD·PDF 객체/텍스트 및 Windows/peets 소스 동일성 검사는 [CONFORMANCE](../CONFORMANCE.md)의 최신 검증에 연결한다. 전체 Mathcad 완성률 20%를 검사 수로 산정하지 않는다.
