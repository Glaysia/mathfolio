# Mathcad 12 구조 수식 편집 명세

[조사 보고서](../analysis/0002-editor-semantics.md)와 [기계 판독 계약](mathcad12-editor-contract.json)은 이 PC의 Prime 12.0.0.1 바이트에서 확인한 사실을 기록한다. 타입 목록을 기능 완성이나 원본과의 동일성으로 계산하지 않는다.

계약에는 94개 literal descriptor 초기화와 직접 확인한 속성·배열 대입, 캐럿 필드, 키 입력 dispatch, 일부 삽입 제약, 분수 배치식이 들어 있다. 각 항목을 원본 SHA-256·MVID·REA Evidence ID·method/field token·IL offset에 연결했다. 생략한 속성은 **unknown**이다. 생략을 false나 0으로 읽지 않는다. 생성자·변환 helper로 만들어진 미해독 값은 `unresolved_explicit_assignments`에 남겼다.

94개는 선택한 두 factory method에서 찾은 literal 초기화 전체다. 제품 전체 descriptor 개수는 아직 모른다. 배열의 직접 대입되지 않은 정수 원소는 CLR `newarr`의 0 초기화에 근거하여 기록했으며, 원본 객체의 다른 기본 속성을 채워 넣지는 않았다. 내부 엔진 코드 생성 템플릿은 이 계약에서 제외했다.

## 독립 구현의 현재 경계

`crates/mathcraft-editor`는 문자열 입력란을 대체하기 위한 **비시각 구조 편집 코어**다. 루트 수식과 append-only arena의 안정 ID, 부모/자식 관계, placeholder, 전체 subtree 선택, 구조적 캐럿, 분수·지수·괄호 삽입, undo/redo를 제공한다.

| 구분 | 현재 내용 |
|---|---|
| 원본에서 확인된 사실 | 노드 종류와 자식 수, descriptor 우선순위, placeholder의 scope 금지, 지수 밑의 fraction/power grouping rule 등록, 별도 caret 상태, fraction 측정/배치식 |
| 독립 구현 정책 | 현재 노드/선택 subtree를 감싸기, 빈 operand의 최초 caret 배치, Space마다 부모 subtree로 확장, 가장 가까운 괄호의 오른쪽으로 나가기, command별 undo snapshot |
| 아직 구현하지 않은 것 | 원본의 전체 키 입력 상태기계, scope의 원본 확장 순서, 부분 operand·matrix-cell typeover, 전체 navigation, 글꼴 측정·hit test·IME, 시각 렌더링, MCDX 편집 상태 왕복 |

공개 API의 `insert_*`는 입력 시퀀스가 해석된 뒤의 구조 조작이다. `/`, `^`, `)` 텍스트를 자동 해석하지 않는다. `=`의 컨텍스트 동작은 계약에 기록했지만 이 크레이트에 일반 이항 평가 노드 삽입으로 위장하여 구현하지 않았다. `insert_binary(Evaluate)`는 명시적 저수준 AST 조작일 뿐 키 입력 `=`의 구현이 아니다.

`Caret.value_range`의 인덱스는 UTF-16 code unit이다. surrogate pair 한가운데는 거부한다. 문자 아래첨자 입력 mode와 matrix index는 별개이며, 문자별 subscript depth를 저장하는 FormattedString 자체는 아직 구현하지 않았다.

`layout_fraction`은 호출자가 실제 font metrics·pad·overhang·bar height를 제공하는 순수 측정식이다. 기본 font 값이나 테마 수치를 발명하지 않았다.

## 검증

테스트는 불완전 operand 보존, 중첩 grouping, 선택 부분만 수정, undo/redo 시 caret와 선택 복원, undo 후 분기에서 ID 재사용 방지, UTF-16 경계, 원본에서 확인한 분수 배치식을 검사한다. 이 테스트 통과는 **우리 구조의 불변식과 명시한 정책**의 검증이며 원본과의 키 입력 호환성 검증은 아니다.

다음 단계는 보고서의 원본 입력 시나리오를 단계별로 수집하여 `required_runtime_probes`를 비교 테스트로 바꾸는 것이다. 원본 결과가 확인되면 독립 정책을 수정하고, 동일성이 확인된 범위만 계약에 실행 근거를 추가한다.

## 구조 수식 위젯

`crates/mathcraft-app/src/formula.rs`의 `formula_ui(ui, editor)`는 arena AST를 직접 측정하고 그린다. 분수·지수·괄호·공통 baseline, 노드 클릭, caret, subtree 선택과 최소 키 입력을 제공한다. 여러 문서 영역에 사용할 때 호출자는 `ui.push_id(region_id, ...)`로 안정적인 ID 범위를 제공한다.

분수 식은 정적 증거에 근거하지만 font 크기·padding·overhang의 실제 값과 exponent/괄호 배치는 독립 정책이다. `=`·matrix index·원본의 모든 navigation·IME 조합은 지원 범위에 넣지 않았다. 원본 앱이나 desktop window를 열지 않고 headless egui frame으로 geometry·hit·tessellation·focus·text input을 검증했다.
