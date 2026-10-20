# 프로그램 로컬 함수의 계산·캡처 경계

2026-10-19. EA-070의 함수 좌변을 설치 도움말·REA descriptor·실제 native Math50 예제로 조사하고 독립 Rust 계산에 연결했다. GUI·스크린샷·원본 프로세스·CLR·COM을 사용하지 않았다. 원본 실행 일치 검사가 아니라 shipped-artifact 정적 증거와 Mathcraft headless 회귀다.

## 원본에서 직접 확인한 범위

`Help/en/PTC_Mathcad_Help/local_assignment_operator.html` SHA-256 `8a66add0cd7fcb5d25f9960d0ea71d0979ff7782419bf33b14eaee81aabd1f30`은 변수와 함수를 프로그램 안에서 정의할 수 있고 worksheet의 기존 값을 참조할 수 있다고 명시한다. 명시적 인자를 가진 함수 선언과 인자를 생략한 함수 alias도 설명한다. 따라서 로컬 함수 자체는 원본에 없는 편의 기능이 아니다. 다만 이번 수치 지원은 **명명된 함수 선언 `f(parameters) ← expression/program`과 그 호출**이다. 함수 값 alias·함수 배열·함수를 일반 값으로 반환하는 기능은 별도의 미구현 범위다.

`about_user-defined_functions.html` SHA-256 `c2138e6f712824f6e3d14b65968bc3cd005bf5b2d04d265b19fc3a19534dfb1c`은 호출 시 실제 인자를 계산하고 formal 인자에 대입한 다음 body를 계산하여 결과를 반환하는 순서를 설명한다. 기존 `Scope::evaluate`의 인자 계산·arity·parameter binding·body 평가·단위/복소수/배열 인자 경로를 재사용할 근거다.

실제 설치 예제 `example_power_regression.html` SHA-256 `d29c422d387003172baed11c5c5ae80bf35189deab3d15700952fb1fe6936333`, textarea `ptc_mathcad_ID0EWCWT`에 `genfitwt` 프로그램이 있다. 이 프로그램은 앞에서 `wt`를 지역 정의하고, 뒤에서 literal suffix를 가진 로컬 함수 이름을 선언한다. 함수의 formal 인자는 `i,v`이며 body는 앞서 만든 `wt`와 outer 인자 `x,f`를 참조한다. 저장은 `<localDefine><function><id>…</id><boundVars>…</boundVars></function>body</localDefine>`다. 즉 outer 인자·앞선 프로그램 local을 읽는 함수 body 및 이름 suffix는 실제 원본 XML로 확인된다.

ignored `artifacts/rea/local-function-source-evidence/original-genfitwt-region.xml`은 해당 region의 원문 5166 bytes를 그대로 보존한다. 원본 HTML byte offset 59565, region SHA-256 `a7583226fb329ec6384b1b6a81c106346d43fc748d5aa04d590d7e80d249d11c`이며 source 파일 identity와 범위는 같은 폴더의 `identity.json` 및 재현 script `local-function-source-ledger.py`에 있다. 원본 예제 전체 계산은 함수형 인자와 `genfit` 등 이번 범위 밖 기능을 포함하므로 실행 성공으로 제시하지 않는다. 검사 소스에 원본 구현 코드를 복사하지 않았다.

## REA descriptor와 미확정 계약

[0085](0085-local-function-authoring-contract.md)의 편집·저장 조사를 참조한다. 새 REA parent `ev_b32cc6da4661d8fd45ed0195ae5e7bbc03d79cab18a651a5aadb5917a763a0d5`는 설치 `Ptc.EquationEditor.dll`의 `DefaultDescriptorTable`을 선택했다. methods view는 `ev_cae14d642514dd67de88ad2bf761cd7e238018b7de1608f5381bea0f67027352`다. DLL SHA `d419032c0e99a62eab2f278e571a338d8c4b9662655af8ab462d482a028c0223`, MVID `d8627dca-3258-470c-b4d1-c00bbe2f2441`를 기존 evidence와 대조했다.

`DefaultDescriptorTable.f`, token `0x06000bbb`, raw IL SHA `90ff4afc15ec4d75068d5d2c0eab08be91548ff38974f8de5b54e02a25a6376a`의 IL 7473은 LOCAL_ASSIGNMENT 좌변 whitelist에 FUNCTION_DEFINITION을 넣는다. 함수 descriptor는 XML `function`, RL의 lambda 표현을 사용한다. 같은 파일의 raw CIL을 dncil로 해석한 기존 `array-slice-authoring-cil.json/.txt`와 SHA를 다시 확인했다. 최신 REA의 method decode는 present이며 selected metadata는 complete다. `#US` 문자열 일부에는 ECMA-335 terminal flag 진단이 있어 해당 문자열 coverage는 partial이다. 가독 문자열/원시 바이트 일치를 원본 런타임 수용성 관찰로 확대하지 않는다.

`front12.lisp`는 lambda를 공통 purifier에 전달하고 recursive-definition 검출 경계도 언급하지만 **로컬 함수 선언 뒤 캡처한 binding을 재대입했을 때 snapshot과 by-reference 중 무엇을 읽는지는 확정하지 못했다.** 위 실제 예제도 캡처 후 `wt`를 다시 바꾸지 않는다. `mccore12`의 인터프리터 연결 및 `msmpl.dll`의 제한된 export 조사만으로는 이 계약이 증명되지 않는다. 따라서 일반 worksheet 함수의 snapshot 경로를 재사용했다는 사실을 해당 재대입 조합에 대한 원본 일치 증거로 쓰지 않았다.

## 구현

`program::Instruction::DefineFunction`은 이름·formal 인자·body를 보존한다. native parser는 함수 구조와 인자 수/중복, metadata를 검사한다. 기존 `Scope::define`으로 이름 검증과 immutable captured scope를 만들고, 호출은 기존 `Binding::Function` 계산 경로를 사용한다. caller의 worksheet와 외부 프로그램에 지역 binding을 출판하지 않는다. 앞선 동일 이름의 함수가 있으면 그 binding을 참조하는 재정의도 가능하며, 자기 출판 때문에 그 backreference를 캡처 변경으로 오판하지 않는다. 아직 정의되지 않은 이름의 재귀·함수형 값은 이번에 지원한다고 주장하지 않는다.

함수 body의 formal 인자는 body 내부 이름 검증에만 추가한다. 프로그램 local 수집·branch/loop 이름 방문도 새 함수를 알고 있어야 하며, formal 인자를 worksheet나 다른 sibling 식으로 누출하지 않는다. 이 label/편집 경계는 [0085](0085-local-function-authoring-contract.md)의 별도 회귀와 함께 검사한다.

캡처 재대입의 미확정 조합에는 명시적인 경계를 뒀다. 프로그램 실행마다 독립적인 mutation ledger를 만들고, 로컬 함수가 실제로 읽는 **free binding만** 등록한다. 계산된 값이 달라진 assignment·array update·loop-variable update가 해당 binding을 바꾸면 이후 그 함수를 호출할 때 원본 capture 계약 미확정 오류를 반환한다. 원본 제품이 이 동작을 거절한다는 뜻이 아니라, 독립 구현이 추측한 숫자를 내놓지 않는 미지원 경계다.

이 ledger는 호출자 parameter 환경을 대조하는 방식이 아니다. 따라서 다음 경우에는 잘못된 변경 오류를 만들지 않는다.

여기서 동일 binding 판정은 일반 수치 `equal` 연산과 구분한다. 실수부·허수부·물리 차원 및 zero-fill 차원은 bit identity로 비교하여 `+0`에서 `-0`로 바뀐 값을 놓치지 않는다. 일반 수치 비교의 `0 = -0` 동작은 그대로 유지한다.

- 관계없는 local을 바꾸거나 같은 수치·단위·배열 값을 다시 대입하는 경우.
- 실행하지 않은 conditional branch 안에 변경문만 있는 경우.
- 함수 formal 인자가 같은 이름을 가리는 경우.
- 함수 body의 중첩 프로그램이 자기 local을 변경하는 경우.
- 다른 로컬 함수를 거쳐 호출하거나 `for` 안에서 현재 iteration 값을 캡처한 뒤 바로 호출하는 경우.

반대로 다른 함수의 captured scope를 거쳐 호출해도 원래 프로그램의 실제 free binding 변경을 숨기지 않는다. 배열 원소·행·열 갱신은 기존 immutable binding을 새로 출판하므로 같은 ledger의 값 비교에 들어간다. 이 비교는 보이는 numeric value뿐 아니라 private zero-fill 차원도 포함한다. 현재 셀 값·단위가 같은 mixed 배열이라도 이후 확장 시 padding 차원이 달라지는 binding 교체를 놓치지 않는다. 함수가 없는 기존 프로그램은 watcher가 비활성일 때 assignment마다 mutex나 snapshot vector를 만들지 않는다. 프로그램별 ledger는 binding 자체를 보관하지 않으므로 function→captured scope와 순환 소유를 만들지 않는다.

함수 선언은 중간 program line으로 가능하지만 마지막 결과가 함수 값이면 기존 숫자를 재사용하지 않고 명시 오류로 처리한다. nested block의 마지막 선언은 나중 program line의 숫자 결과로 대체할 수 있어 불필요하게 전체 블록을 거절하지 않는다. 취소 토큰은 captured scope에 보관하지 않으며, body 실행은 기존 호출 깊이와 cooperative cancellation 경로를 유지한다.

## 검증과 남은 범위

`tests/local_function_calculation.rs`의 11개 headless 회귀는 outer 인자/앞선 local capture, caller 격리, parameter shadow, 다른 로컬 함수 경유, 실제/동일값/무관한 재대입, 실행하지 않은 branch, 중첩 프로그램 local, 이전 함수 backreference, 단위·복소수 인자, 선언/arity/함수값 오류, loop 내 현재 값 capture, indexed/slice array 변경, 160단계 local call의 깊이 한계를 검사한다.

private `program::local_function_tests::cancelling_a_local_function_call_preserves_the_caller_and_fresh_retry`는 함수 body loop 중간에 취소한 뒤 caller binding의 Arc identity·program-frame 부재·함수 비누출을 확인하고, 새 control에서 재실행하여 3000을 얻는다. 기존 일반 프로그램·worksheet 함수·수식/배열·saved result 회귀도 함께 실행했다.

최종 Windows compat 검사는 **249 passed / 0 failed / 4 ignored**, all-targets Clippy `-D warnings`도 통과했다. ignored 로그는 `local-function-compat-tests-20261019.log`, `local-function-compat-clippy-20261019.log`다. 양OS·앱 native MCDX/PDF 통합은 [0087](0087-local-function-integration.md)을 따른다. 함수 값 alias·함수 배열·higher-order/함수 값 반환·완전한 재귀·변경된 캡처의 원본 동작·모든 branch/loop scope 조합·정확한 원본 오류 문구는 미완료다. 이 범위의 검사 수를 제품 완성률로 환산하지 않는다.
