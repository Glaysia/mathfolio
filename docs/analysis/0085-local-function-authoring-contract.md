# 프로그램 내부 함수 선언의 구조·저장·label 범위

2026-10-19. LocalDefine의 왼쪽에 native Function 선언을 두는 계약을 조사·연결했다. GUI, 스크린샷, 원본 앱, CLR, COM은 실행하지 않았다. 수치 함수 호출·정의 시점의 capture 및 local scope 계산은 계산 조사에서 다룬다. 함수값 alias나 고차 함수까지 구현했다는 보고가 아니다.

## 원본의 긍정 증거

설치 `Help/en/PTC_Mathcad_Help/local_assignment_operator.html` SHA-256 `8a66add0cd7fcb5d25f9960d0ea71d0979ff7782419bf33b14eaee81aabd1f30`의 source line 54와 60은 프로그램의 local assignment가 변수와 함수에 적용됨을 설명한다. line 63–65는 바깥 worksheet 값 참조, 명시적인 parameter 목록을 가진 local 함수, 함수값 alias 및 같은 이름의 worksheet binding을 local 값이 가리는 경우를 설명한다. 이 중 이번 저작 확장은 **명시적인 Function name/parameters를 가진 선언**이다. 도움말의 alias 예제 전체를 이번 구현으로 확대하지 않는다.

실제 native XML도 확인했다. `example_power_regression.html` SHA `d29c422d387003172baed11c5c5ae80bf35189deab3d15700952fb1fe6936333`의 textarea `ptc_mathcad_ID0EWCWT`에는 Program 내부 `localDefine`의 LHS로 `function`이 저장된다. 이름 `f`의 literal suffix `wt`는 XAML Span/Subscript이며 formal `i`, `v`는 `boundVars` 아래의 VARIABLE id다. 이 증거는 함수 선언의 native 구조를 직접 뒷받침하며 그 power regression 예제 전체나 함수값 formal까지 계산할 수 있다는 검증은 아니다. 두 파일의 path·byte length·SHA는 ignored `local-function-authoring-help-identities.json`에 보존했다.

같은 계약을 REA descriptor에서 교차 확인했다. `Ptc.EquationEditor.dll` SHA `d419032c0e99a62eab2f278e571a338d8c4b9662655af8ab462d482a028c0223`, MVID `d8627dca-3258-470c-b4d1-c00bbe2f2441`, `DefaultDescriptorTable.f` token `0x06000bbb`, raw IL SHA `90ff4afc15ec4d75068d5d2c0eab08be91548ff38974f8de5b54e02a25a6376a`를 사용한다. 새 parent evidence는 `ev_b32cc6da4661d8fd45ed0195ae5e7bbc03d79cab18a651a5aadb5917a763a0d5`, method view는 `ev_cae14d642514dd67de88ad2bf761cd7e238018b7de1608f5381bea0f67027352`다. 일부 #US string의 terminal flag 경고는 readable text와 method-byte identity를 잃었다는 뜻으로 해석하지 않았으며, 원본 실행을 통해 runtime acceptance를 확인한 것도 아니다.

LOCAL_ASSIGNMENT는 IL 7244에서 XMLFormat `localDefine`를 설정한다. LHS whitelist IL 7446–7530에는 IL 7473의 FUNCTION_DEFINITION이 명시적으로 포함된다. IL 7369–7402에는 APPLY를 FUNCTION_DEFINITION으로 바꾸는 규칙도 있다. 따라서 worksheet의 일반 define과 구분되는 local 함수 선언 자체는 원본 기능이다. descriptor의 자동 변환·부분 operand 선택 동작을 모두 재현했다는 보고는 아니다.

writer dispatch와 operator 저장을 확인한 raw method identity는 [0081](0081-array-slice-definition-contract.md)의 같은 DLL/token에 이어진다. 프로그램 함수를 일반 `define`이나 임의의 `programFunction` 태그로 바꾸지 않는다.

```xml
<ml:program>
  <ml:localDefine>
    <ml:function><ml:id>f</ml:id><ml:boundVars><ml:id>x</ml:id></ml:boundVars></ml:function>
    <ml:apply><ml:plus/><ml:id>x</ml:id><ml:real>1</ml:real></ml:apply>
  </ml:localDefine>
  <ml:apply><ml:id>f</ml:id><ml:real>4</ml:real></ml:apply>
</ml:program>
```

## 독립 편집·bridge 경계

기존 `NodeKind::Function { parameter_count }`의 child 0은 이름, 이후 child들은 formal이다. 새 함수 AST나 public document 형식을 만들지 않았다. PortableTree의 LocalDefine LHS 허용 목록에 Function을 추가하고 이름·formal은 identifier 또는 편집 중의 Placeholder로 제한했다. ordinary worksheet Function의 기존 저장 구조는 유지한다. For의 binder는 여전히 이름이며 Function이나 Call을 binder로 허용하지 않는다.

native function reader는 기존대로 id name과 `boundVars`의 id parameters만 받는다. native writer는 LocalDefine Function의 이름·formal이 모두 완성된 identifier여야 하며 Program host도 요구한다. 미완성 formal은 editor draft와 Undo/Redo에 남길 수 있지만 native XML로 조용히 잘못 저장하지 않는다. Call AST를 선언으로 간주하지 않으며 함수명·formal의 explicit label, contextual flag와 literal suffix를 보존한다. 인자 개수·중복 formal 및 수치 바인딩의 유효성은 기존 함수 계산 계약과 계산 담당 검증을 따른다.

기존 `insert_local_definition()`은 현재 operand를 RHS에 넣는 template였으므로, `f(x)`를 쓴 다음 local assignment를 삽입해도 함수 선언이 되지 않았다. 원본의 APPLY→FUNCTION_DEFINITION 규칙과 child 0의 선언 whitelist를 근거로 이 경로를 고쳤다. Program statement에 닫힌 named Call이 있으면 같은 node를 Function으로 바꾸어 LHS에 보존하고 RHS Placeholder로 진입한다. 현재 target이 이름·완성 Function·named Index/Row/Column이어도 같은 LHS 방향을 적용한다. 빈 statement에서는 LHS Placeholder에 남아 이름부터 작성할 수 있다. 변환과 wrapper 생성은 한 checkpoint다.

이번 helper의 지원 위치는 기존 Program의 직접 statement target이다. 닫히지 않은 call의 내부, 함수 formal, For binder나 일반 식의 일부에 local assignment를 중첩하지 않는다. 비Program, matrix range, 잘못된 base 또는 수치 formal을 가진 Call에서는 tree·revision·history를 바꾸지 않는다. 이는 invalid 구조를 만들지 않기 위한 독립 구현의 지원 경계이며 원본의 모든 부분 operand 선택과 자동 pull-out을 복원한 것은 아니다. 함수값 alias, 함수값 배열, 함수 parameter로 함수를 전달하는 고차 함수도 이번 확장의 계산 지원 범위가 아니다.

상위 formula 입력도 설치 `Doc/en-US/keyboard_shortcuts.pdf`의 18쪽 Local Assignment 항목에 맞춰 `{` 문자를 이 helper에 연결했다. headless 입력 회귀는 Program에서 `f(x){x^2`를 입력하고 새 줄에서 `f(3)`을 호출하여 native MathML 왕복 뒤 9를 계산한다. 비Program 입력은 무변경이고 literal suffix 안의 `{`는 suffix 문자로 남는다. 앱 transaction 회귀는 local 함수 body를 수정하는 동안 계산하지 않다가 Enter로 확정하며 Undo로 이전 계산을 복원한다. native preflight는 6 regions·5 cached query displays·저장본 재열기 실행 0·native 재계산 오류 0을 확인했다. 이 CPU 검사에서는 GUI를 띄우지 않았다.

## 함수 parameter의 label scope

이전 label resolver는 한 region의 모든 `boundVars`를 하나의 overlay에 모았다. local `f(m)`을 추가하면 그 parameter 이름 `m`이 뒤의 다른 local 함수와 Program 바깥 식에서도 VARIABLE이 되는 문제가 있었다. 모든 명시 함수의 formal은 그 소유 `define`/`globalDefine`/`localDefine` node에, calculus binder는 그 `lambda` node에 overlay를 둔다. identifier의 ancestor에서 가장 가까운 overlay부터 찾아 outer 함수의 formal은 중첩 Program 안에서도 참조할 수 있고 sibling 함수의 formal은 새지 않는다.

프로그램 local assignment 이름은 [0081](0081-array-slice-definition-contract.md)에서 연결한 Program별 overlay를 유지한다. 명시적인 label은 다시 추론하지 않고 contextual label만 현재 scope에 맞게 해석한다. `f(m)`의 `m`이 cached UNIT이면 formal과 그 body에서 VARIABLE이 되지만, sibling `h(s)`의 body와 Program의 나머지 식·바깥 식의 `m`은 단위 의미를 유지한다. 이 정책은 독립 구현의 bounded label 선택이며 원본의 모든 overload·type inference를 복원한 것은 아니다.

## 검사

editor 신규 회귀 네 개는 native identifier metadata를 가진 Function LHS 삽입과 단일 Undo/Redo, formal 추가 중 Placeholder 유지, 잘못된 name/formal과 변환하지 않은 Call LHS 거절, For binder 제한을 검사한다. 새 helper 회귀는 이름·Call·Function·Index·Row·Column·빈 statement의 LHS 방향과 focus, 동일 NodeId 보존, 원래 Call까지 복원하는 단일 Undo/Redo, 비Program·statement 내부·잘못된 target의 무변경을 확인한다. editor 전체 72개가 통과했다.

`tests/local_function_bridge.rs`의 네 회귀는 native local Function roundtrip·literal suffix·contextual metadata, 미완성 formal writer 오류와 잘못된 native Function/Call 거절, sibling 및 외부 scope로 parameter label 누출 방지, outer worksheet 함수와 nested local 함수·calculus lambda의 각각 다른 binder scope를 검사한다. focused 4개와 editor/compat clippy `--all-targets -- -D warnings`가 통과했다. 전체 compat 검사와 app 저장·열기·수치·PDF·Windows/peets 검증은 상위 통합 기록을 따른다.

ignored 로그는 `local-function-editor-tests.log`, `local-function-bridge-tests.log`, `local-function-compat-all-tests.log`, `local-function-editor-bridge-clippy.log`, `local-function-editor-authoring-clippy.log`, `local-function-app-focused-20261019.log`, `local-function-native-preflight-20261019.log`에 있다. 전체 Mathcad 진척률이나 GUI 조작의 동등성을 이 회귀 개수로 환산하지 않는다.
