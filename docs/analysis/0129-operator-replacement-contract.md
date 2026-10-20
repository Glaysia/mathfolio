# 기호 중심에서 연산자 교체

2026-10-19. 설치 Mathcad Prime12의 정적 REA와 원시 CIL을 독립 Rust 입력 경로에 연결한다. GUI·스크린샷·원본 실행·CLR/COM은 사용하지 않는다. 일반 문자 분할 입력은 [0118](0118-normal-infix-entry-contract.md), 삭제는 [0122](0122-structural-deletion-contract.md)의 별도 경로다.

## 근거와 분기

E 설치 DLL SHA256 `d419032c0e99a62eab2f278e571a338d8c4b9662655af8ab462d482a028c0223`, MVID `d8627dca-3258-470c-b4d1-c00bbe2f2441`. 이번 REA member parent는 `ev_3c23753e609d06f6586eab52fb93064c8bcef787b29309269b984ebf062c79e1`, methods view는 `ev_19cf7bc77eb32906ee49f55db62511307e2db490f4721a2056640545c26af8a6`다. NearestNeighborSelector 별도 parent는 `ev_d7a74cee6f464f2b5d1298ed5a2e471d9c674e0bed72112a8437dc3b77d5d445`다. selected type와 metadata는 complete이며, #US의 terminal flag 차이는 tool의 partial 한계로 보존한다. 원시 method body는 설치 bytes와 별도 decoder로 대조했다.

Normal l0088(`d2e82ba51ec7275cbe5e6b911eaf06401ff6a634851661ae6462fb31e78c74e6`) IL022a..02a3는 WhatSideOfTheNodeIsTheCaret=Center·NodeSide=Center에서 ax04ed로 교체 명령을 보낸다. 값의 문자 범위를 분할하는04d9, Scope의 피연산자 wrapping과 다르다. ParenthesizeOperands 설정을 이 명령에 전달한다. C 설치 property229의 DefaultSettingValueAttribute는 `True`이며 UserScoped라 실제 사용자 override 값이라고 주장하지 않는다. metadata 증거는 `typeover-setting-defaults-20261019.json`이다.

bi05cc→a2.052d는 교체 handlers를 순서대로 검사한다. 기본 ParenthesizeOperands의 flag1에서 a2.m0533(`bd6a2310f1d7787e3d34802ad96a286218c381f35c108cd39250c10584dbdd71`)는 기존/새 descriptor의 ChildCount·Fixity·IsLeafOperator 호환성을0541로 검사한다. 일반 이항 infix는 이에 해당한다. 다른 fixity·nary·특수 연산자와 IgnoreOperandsOnOvertype는 별도 handlers를 거치므로 같은 두 자식 정책을 일반화하지 않는다.

## 가까운 피연산자와 우선순위

0533은 먼저 NearestNeighborSelector1069/106a(`7bd909b58fbcf8fa6c5fd4c2dff86bdc9a07b48d594e4fdbddfcb8efc71c01a4` / `2c4ee33f3edb9a0c0ff87e32f68ff9aeec1eb7ad0531fcf30b8539b5ffc84ac6`)로 피연산자를 고른 뒤 descriptor를 바꾼다. Infix의 오른쪽은 기존 child1, 왼쪽은 그 노드의 LeftAdjacentSubexpression(true)이며 없으면 child0다. c8c의 last-child 분기는 container slot을 넘지 않는다. 왼쪽 식의 우선순위가 더 높으면 전체를 보존하고, 같은/낮은 우선순위의 왼쪽 결합 infix chain이면 마지막 인접 항을 고른다.

따라서 `1+2+3`의 두 번째 `+`를 `*`로 바꾸면 `1+2*3`이다. 전체 왼쪽 subtree를 무조건 보존해 `(1+2)*3`으로 만들면 다르다. 0533 IL0097..0102는 이때 기존 operator node를 가까운 두 피연산자 사이로 재배치한다.

그 뒤 old container slots를 정규화하고, 새 일반 operand의 우선순위가 더 낮으면 Group을 추가한다. 더 강한 부모 아래에 있는 새 연산자도 container가 아니면 Group으로 보호한다(0533 IL017d..0207). 마지막으로0582의 container parser/repair를 수행하고 기존 operator의 center navigator를 반환한다. 단순 descriptor 변경이나 무조건 전체 flat parse만으로는 두 기본 규칙을 함께 지킬 수 없다.

## 독립 구현과 검증 범위

`operator_replacement.rs`는 현재 표현되는 일반 이항 infix와 Fraction·Power의 같은 arity 교체를 새 Normal center 경로에 연결한다. Power의 실제 descriptor는 ChildCount=2·Fixity=Infix·nonLeaf이며, 분석 목록의 EffectivePostfix와 다르다. `typeover-power-replacement-guards-cil-20261019.txt`에서0541의 세 비교와 IgnoreOperandsOnOvertype=false를 확인했다. `-` 입력은 여기서 이항 Subtract를 선택한다. 가까운 항 재배치·기본 grouping·container 재파싱을 후보 arena 안에서 수행하고, 유효성·예산 검사 후 한 번 checkpoint한다. 같은 연산자나 거절된 후보는 source·caret·redo를 유지한다.

코어 회귀는 root 교체의 두 피연산자/ID/center/Undo, chain의 인접 항, 더 강한 Power base 부모 아래의 Group, 이항 minus와 거절 시 redo를 검사한다. 실제 CPU 앱 Text 이벤트의 `*`, `-`, `/`, 비교 입력도 검사한다. 전체 앱의 Enter/저장·MCDX·PDF와 양 OS 검증은 후속 통합 기록에 연결한다.

Power↔Add/Fraction, 새 지수 container의 복합 RHS, 실제 `^` 입력과 native 계산을 검사한다. 모든 prefix/postfix·PairNary·APPLY 교체, ParenthesizeOperands=false 사용자 설정, Fraction·Power의 container 해제 시 같은 우선순위 복구와 모든 metadata 변환은 아직 구현 범위 밖이다. 이를 원본 전체 연산자 교체 완료로 보고하지 않는다.

새 원시 검증 자료: `artifacts/rea/operator-replacement-{rea-summary,fresh-methods,verified}-20261019.json`, `feedback/operator-replacement-verified-20261019.txt`, byte 검증 script/log. 추가 bi/flag/handler 순서/0582/c8c는 기존 REA metadata의 동일 artifact SHA와 설치 body SHA로 재검증하며, 새 parent의 선택 범위에 포함됐다고 잘못 표시하지 않는다.
