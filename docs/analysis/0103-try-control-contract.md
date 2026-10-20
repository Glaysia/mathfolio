# Try / on error 원본 계약 — 2026-10-19

원본 Prime 12.0.0.1의 정적 REA metadata/CIL, Math50 schema, 동봉 도움말로 TRY의 저작·직렬화·조판 계약을 조사했다. 원본 앱, GUI, 스크린샷, CLR, COM은 실행하지 않았다. 조사 상세는 ignored `artifacts/rea/try-next-contract-20261019.md`에 보존했다. 이 문서의 수치 실행 정책은 원본에서 확인한 사실과 독립 구현의 선택을 구분한다. 제품 코드의 검증 결과는 별도 구현 보고서에서 다룬다.

## 근거 신원

설치 기준은 `C:/Program Files/PTC/Mathcad Prime 12.0.0.1/`다. 기존 `artifacts/rea/editor/{equation,control}-{members,il}.json`을 재사용하고 `feedback/label-cycle-inspect.py`로 설치 파일 SHA 및 선택한 메서드의 원시 IL SHA를 REA와 재대조했다.

| 기호 | 파일 | SHA-256 / MVID / REA Evidence |
| --- | --- | --- |
| E | `Ptc.EquationEditor.dll` | `d419032c0e99a62eab2f278e571a338d8c4b9662655af8ab462d482a028c0223` / `d8627dca-3258-470c-b4d1-c00bbe2f2441` / `ev_419ab24312df266f2e58ff6b91d32e0261a9a2bba9105ba09ebecb7517b43fdf` |
| C | `Ptc.Controls.EquationControl.dll` | `0b6fd580eb6cc183f17b5b5e0967226bbafc5e12efc6da809f44cd16416bf9d1` / `cf1d7cb5-c73f-4618-8e6b-b83959d13b98` / `ev_389338a9845c7ea83dfd310922316cf92ca1637bdbac6602a01c4690e52b8349` |

| 동봉 자료 | SHA-256 |
| --- | --- |
| `Help/en/PTC_Mathcad_Help/try_operator.html` | `e91b451efe60a19f8211910d8f56223195de5e641737d6012dc5637f2fecf68b` |
| `Help/en/PTC_Mathcad_Help/example_using_the_try_on_error_operator.html` | `9448967dd8b905eb5980cacb360e216a0a53319917c9e45a7411c9548295b631` |
| `Help/en/PTC_Mathcad_Help/Tutorials/prgm_tutorial/task3-3_Writing_Try_On_Error_Statements.html` | `3b7dc2dd9dc854f87965f29eb2308134a3d07c9b2481a02c3bf8dd7926c58793` |
| 추출 `artifacts/rea/document-deep/schemas/Math50.xsd` | `a10afb4dd442a4f5dd77b6f1df903c1d2e5b6f4d6a701aae38b0cefee21f6ca3` |

## TRY는 순서가 정해진 두 BODY를 갖는다

E `DefaultDescriptorTable.f` `0x06000bbb IL_4c78..4dfb`는 descriptor `TRY`, XMLFormat `tryCatch`, RLFormat `(try:0x{1:x8} {2} {3})`, S-expression `@TRY`, OperatorText `try`, special fixity 5, child count 2, precedence 290을 선언한다. **child0은 시도할 body, child1은 on-error body**다. 두 child 모두 PROGRAM이면 descriptor를 BODY로 교체하며 그 밖의 식은 BODY parent로 감싼다(`4d5e..4df3`). BODY의 native `<program>` 및 왼쪽 바 조판 계약은 [0099](0099-conditional-branch-contract.md)와 같다.

| 속성 | 원본 값 / E initializer IL |
| --- | --- |
| PrimalChildPosition | 0 / `4cd1..4cd9` |
| NavigablePartCount / positions | 2 / [0,1] / `4cb8..4ccc` |
| InherentChildren | **[0,1]** / `4cde..4cea` |
| ContainerSlots | [0,1] / `4d0b..4d17` |
| NavigatablePartRows | [0,2] / `4d38..4d44` |
| ChildrenRows | [1,3] / `4d49..4d59` |
| 양쪽 caret / vertical navigation / jump into children | 모두 허용 |

정오표: 이전 ignored 중간 기록 `artifacts/rea/program-branch-next-contract-20261019.md`의 TRY inherent [1] 문구는 잘못이다. `4cde..4cea`는 Int32[2]를 만들고 element1=1로 설정하므로 element0의 기본값 0과 합쳐 **두 child 모두 inherent**다. 이전 artifact는 조사 이력으로 보존하고 이 정오표를 따른다. IF의 [1]과 혼동하지 않는다. TRY 삭제에는 별도 E `ap.aa` `0x06000440` 경로가 있으며 empty/caret 조건에 따라 두 body merge와 이전 Program merge를 수행한다. break/continue의 0-arity 삭제 계약을 그대로 적용할 근거는 없다.

E writer 등록 `0x06001167 IL_0b55..0b75`는 TRY를 `TransformDefaultNode` `0x06001173`으로 보낸다. XMLFormat으로 element를 만들고 child 순서를 유지한다. generic reader `0x06001265 IL_0255..0269`도 일반 child decode 후 descriptor를 생성한다. Math50.xsd 2143~2157행은 tryCatch 아래 expressionClass를 정확히 2개 요구하고 try 다음 catch임을 설명한다.

```xml
<ml:tryCatch xmlns:ml="http://schemas.mathsoft.com/math50">
  <ml:program><!-- attempted statements --></ml:program>
  <ml:program><!-- on-error statements --></ml:program>
</ml:tryCatch>
```

별도 `<try>`, `<catch>`, `<onerror>` wrapper를 발명하지 않는다. schema는 두 operand를 program으로만 제한하지 않으므로 scalar native body도 원본 generic reader+BODY repair에 맞춰 한 statement Program으로 정규화할 수 있다. 모든 중첩 Program을 flatten하는 근거는 없다.

## 도움말로 확인한 계산 의미

동봉 Try 도움말은 먼저 x를 평가하고 성공하면 x를 반환하며, 계산 오류가 나면 y를 평가·반환한다고 설명한다. x와 y 모두 식 하나 또는 여러 식의 연속일 수 있다. local assignment, 함수 호출, 프로그램 단계도 허용한다. handler는 오류 없이 평가되는 식이어야 한다.

**0으로 나눌 때 handler로 넘어가는 동작은 명시적으로 확인됐다.** Try 연산자 도움말과 tutorial은 division-by-zero를 infinity fallback으로 처리한다. 오류 처리 예제의 embedded native XML도 다음을 담는다.

| region ID | attempted body | handler body | 도움말의 제어 결과 |
| --- | --- | --- | --- |
| `ID0EJTLS` | localDefine | localDefine, break | 현재 inner loop 종료 |
| `ID0E13LS` | localDefine | localDefine, continue | 현재 loop의 다음 반복 |
| `ID0EV6LS` | localDefine | localDefine, return | 전체 program에서 값 반환 |

따라서 handler의 break/continue/return은 원래 loop/program 제어로 전파해야 한다. 이들을 오류로 catch하거나 일반 값으로 바꾸면 안 된다. handler의 local assignment가 현재 program의 matrix에 반영되는 사례도 확인된다. 별도 exception 변수나 error object binding은 이 계약에 없다.

다음은 **미확정**이다: undefined identifier가 사전 검사에서 거부되는지 실행 중 catch되는지, dimension/type/index/invalidargument/custom error별 정확한 catch 대상, cancellation·host abort·메모리 부족 처리, 부분 성공한 local assignment 뒤 다른 식이 실패할 때 rollback 여부, nested handler 오류의 원본 chaining 방식, compile/type/name validation이 lazy handler와 상호작용하는 정확한 방식. 도움말의 일반적인 계산 오류 설명만으로 구체적인 오류 코드 whitelist를 복구했다고 할 수 없다.

동봉 `lisp/front12.lisp` SHA `5b15dd2d9066b253200133a5b5fd79263718a9f5f5d4e520cdadda5c104e01ee`의 `d-handle-errors`/`emonad-catch`는 front-end worksheet monad 처리다. **프로그램 TRY 구현이 아니므로 거기서 catch하는 Lisp error class를 TRY 전체에 적용하지 않는다.** 기존 mccore12/mcfront12/eficore 선택 metadata에서도 프로그램 Try 구현을 확인하지 못했으며 이 선택 결과는 전체 numerical engine에 대한 부재 증명이 아니다. `onerror()`라는 별도 함수 alias를 추가할 근거도 없다.

### 독립 엔진의 제한된 정책

이번 구현은 계산 가능한 기존 worksheet fault를 handler로 보내는 명시적인 classifier를 사용한다. 성공한 attempt의 값/제어 결과는 그대로 반환하고 handler는 평가하지 않는다. catch 대상 오류가 발생하면 handler를 한 번 실행한다. handler 자체의 오류는 바깥으로 전파하며 outer Try의 대상이 될 수 있다. 이 nested 전파 선택은 원본의 모든 오류 chaining을 복구한 결과로 주장하지 않는다.

`unsupported=true`, 기존 region에서 전파된 오류, `CANCELLED`, `program_budget`, 잘못 놓인 제어/값 없음, 구현 범위·리소스 상한은 일반 fallback으로 숨기지 않는다. **미구현 함수가 Try 때문에 성공한 것처럼 보이면 안 된다.** division-by-zero 외의 dimension/index/argument 등의 처리 범위는 기존 독립 엔진의 제한된 계산 정책이며 코드별 원본 실행 관찰이 아니다.

attempt와 handler는 같은 imperative local Scope를 사용한다. 새로운 rollback을 발명하지 않으며 program 외부 worksheet Scope의 기존 복제 경계는 유지한다. 원본에서 실패 전 일부 local 상태를 되돌리는지는 아직 미확정이므로 같은 Scope 정책을 원본과 완전 일치한다고 주장하지 않는다. 함수의 사전 name/capture 검사가 양쪽 body를 걷는 기존 경로도 별도 한계다. 코드의 상세 classifier와 검증 결과는 해당 구현·회귀 테스트가 최종 근거다.

## Ctrl+J·직접 저작과 standalone 경계

도움말은 `try`를 입력하고 Ctrl+J를 눌러 모든 placeholder를 가진 연산자로 바꿀 수 있다고 명시한다. 리본 경로는 Math → Operators and Symbols → Programming → try다. E `a8.bq` `0x0600059e`의 Normal mode, IdentifierNodeInfo, HasNoLabel(null/None), exact whole formatted identifier 비교 조건은 [0096](0096-program-control-authoring-contract.md)·[0099](0099-conditional-branch-contract.md)와 같다. TRY는 `try`라는 OperatorText의 단일 descriptor이며 IF/IF_THEN의 중복 단어 문제가 없다. suffix를 무시한 base-only 변환, 강제 lowercase, loop-only gate를 추가하지 않는다.

빈 단어 변환은 기존 단어를 남기는 wrapper 대신 TRY의 두 placeholder body를 만들고 primal child0으로 이동하는 구조 삽입·repair다. 같은 저작 명령은 한 Undo로 tree/caret/selection/scope를 복원해야 하며 실패 시 history와 redo를 바꾸지 않는다. populated target 전체의 split/merge 계약은 아직 별도 범위다.

TRY descriptor는 Program parent/ancestor를 강제하지 않고 tryCatch는 schema expressionClass에 속한다. 원본 도움말의 확인된 native 사례 4개는 모두 Program 내부다(위 세 사례 및 regression 예제 `ID0EHF4W`: Program→Then→If→Program). **eval/define 바로 아래의 Try를 원본에서 실행한 사례는 확보하지 못했다.** root Try를 독립 계산하기 위해 `Expression::Program`의 한 Try statement로 평가하는 지원은 schema와 허용된 저작을 연결하는 제한된 구현 선택이다. 원본의 implicit Program repair를 수치 실행으로 증명한 것으로 쓰지 않으며 이 판단을 standalone For/If 전체로 확대하지 않는다.

## 원본 조판은 네 행이다

C factory `0x060006a9` → VisualTryNode constructor `0x06000d67`는 표시 문자열 **`try`**, **`on error`**와 opacity0.3의 오른쪽 single minor fence를 만든다. `catch`, `onerror`, `then`을 표시하는 근거는 없다.

C Measure `0x06000d73`의 배치는 try header x0 → child0 BODY x8 → on error header x0 → child1 BODY x8 순서로 각 앞 행 높이를 더해 아래에 쌓는다(`IL_01b5..0249`). content width는 max(두 header 폭, 8+두 body 최대 폭), height는 네 요소 높이의 합이다. 오른쪽 fence는 content.width, y0이다(`024e..0279`). Arrange `0x06000d74`는 그 위치를 사용한다. BODY는 [0099](0099-conditional-branch-contract.md)의 왼쪽 double glyph 바를 유지하며 자체 오른쪽 minor fence는 생략한다. 폰트/DPI/glyph의 완전한 조판 일치는 아직 별도 검증 대상이다.

## 선택한 원시 IL SHA-256

| 어셈블리·token | 역할 | IL SHA-256 |
| --- | --- | --- |
| E `0x06000bbb` | descriptor initializer | `90ff4afc15ec4d75068d5d2c0eab08be91548ff38974f8de5b54e02a25a6376a` |
| E `0x06001167` | writer dispatch | `3930351561a894c121a700a2d9fa21cb2d5c8966dbba30f0550897f7c2136e89` |
| E `0x06001173` | TransformDefaultNode | `2749dcc2ffbbef2b8312c385799049d9b52c22f2f2a32b257be339145e6251b2` |
| E `0x06001265` | generic reader | `c3f80d734a3bd2bbe929d2c505e7323010d9ba0d5421e86119bf6ff35357d40a` |
| E `0x0600059e` | Ctrl+J | `dbaea23c9588a86b123e3b7d67e5b4c86467ce28b38ef1873559dd97a2bb9f0c` |
| E `0x06000440` | Try deletion | `9a27b217ca9d0376d5042857a97e45dd129f769af05ac71d2f6cb0676fc00e4a` |
| C `0x060006a9` | visual factory | `03e333699bd2e2881e4789abc7a64fcdfe6084e92241be151aed6b9e7bdf5e46` |
| C `0x06000d67` | visual constructor | `cf1aade6cffd295c3c37c745a5af8548c0a36734a6ca1d31038a7f9c2934d3dc` |
| C `0x06000d73` | Measure | `65a62889195522c21983cac11075ec977997680cddb4192e4ffa2c514cfcf8b9` |
| C `0x06000d74` | Arrange | `f5a3ac1b37fa9627346cf4405310ad0ebd2eb05aabb41ade5320eb564a7e5f3d` |
