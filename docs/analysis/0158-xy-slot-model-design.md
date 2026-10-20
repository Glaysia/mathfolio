# XY 슬롯 삽입·삭제의 최소 문서 모델

2026-10-19. [0157](0157-native-xy-slot-authoring-contract.md)의 설치 CIL 계약을 [r156 모델](0156-shared-xy-axis-integration.md)에 적용하는 설계다. 이 문서의 첫 작성 시점에는 제품 Rust를 편집하지 않았다. 이후 순수 모델 구현·검증 결과는 마지막 절에 별도로 기록한다. GUI·스크린샷·원본 runtime을 사용하지 않는다. 새 appearance 선택, 키·포커스·Undo UI와 계산 실행은 순수 모델 밖이다.

## 보존해야 하는 세 가지 순서

식 순서, model trace의 global append order, 각 식의 mutable trace membership order는 서로 다르다. EquationItem10a7은 List를 그대로 반환하고10a8/10a9는 Add/Insert를 한다. membership의 first/Skip(1)를 매번 global 순서로 대체하면 원본 연산을 바꾼다. 원시 본문은 `artifacts/rea/feedback/native-xy-slot-membership-cil-20261019.txt`다. 이 구분은 [0155](0155-shared-xy-axis-model-design.md)의 source identity와도 별개다.

native writer1154가 사용하는 SortedTraces1153는 X count>Y count이면 X, 그 밖에는 Y의 equation ordinal을 key로 global trace를 stable OrderBy한다. 따라서 앞에 식을 넣어 새 trace가 model 끝에 append돼도 native trace/appearance ordinal은 앞일 수 있다. 이 정렬 양성과 membership List 양성을 함께 사용해야 한다. 이 문서가 직접 확인한 원시 파일은 `native-xy-slot-trace-order-cil-20261019.txt`, `native-xy-pairing-presenter-cil-20261019.txt`, `native-xy-slot-add-remove-cil-20261019.txt`다. 정확한 설치 identity·IL hash와 이후 보강은0157 ledger를 따른다.

## 현재 subset에서는 Graph를 추가하지 않는 이유

지원 전제는 두 primary 축이 비어 있지 않고, trace 수 L=max(nx,ny)이며, sorted trace T[i]가 각 축 min(i,count−1)에 연결되는 기존 모델이다. 여기서 각 축 membership은 **singleton prefix + 마지막 슬롯의 ordered suffix**다. global append order는 임의여도 된다. 비primary 축, legend reassociation, 별도 trace-axis association이 있는 원본, 비canonical membership은 이 전제에 포함하지 않는다. 그런 데이터를 이 API에 조용히 정규화하지 않는다.

다음 네 분기는 원본 List 연산과 SortedTraces로부터 도출한다. n은 수정할 축의 기존 count, m은 반대 축 count, k는0기준 위치다.

| 연산 | trace 변화 | appearance 변화 |
| --- | --- | --- |
| 삽입, n>=m | 새 trace1개 | 새 appearance를 ordinal k에 insert |
| 삽입, n<m | 기존 trace 재연결 | 기존 appearance 순서·값 그대로 |
| 삭제, n>m | 선택 식의 첫 trace1개 제거 | ordinal k appearance remove |
| 삭제, 1<n<=m | 기존 trace 재연결 | 기존 appearance 순서·값 그대로 |

귀납의 핵심은 다음과 같다. 신규 trace가 필요한 삽입에서 반대 membership min(k,m−1)에 max(0,k−(m−1)) 위치로 넣는다. k가 singleton prefix 안이면 새 trace를 먼저 남기고 기존 trace를 다음 membership 앞쪽으로 연쇄 이동한다. 마지막 suffix 안이면 그 suffix의 정확한 상대 위치에 들어간다. 이는 sorted trace 목록에 새 trace를 k번째로 삽입한 결과다. shorter 삽입은 빈 슬롯이 successor first를 인수하거나 기존 마지막 suffix를 나누므로 trace 순서를 바꾸지 않는다. longer 삭제는 singleton 하나를 제거하고 반대 축의 빈 membership을 successor first로 채운다. shorter 삭제에서 중간 슬롯은 singleton이므로 prepend가 여러 trace의 역전을 만들지 않는다. 여러 trace를 가진 마지막 슬롯 삭제는 이전 마지막 슬롯 뒤에 append한다.

따라서 지원되는 완료 상태에서 longer 축의 equation마다 trace가 정확히1개이고 SortedTraces key에 tie가 없다. equal-count에서 Y를 택하는 전환도 양 축이 singleton이라 같은 trace 순서를 만든다. global append order의 차이가 사라졌다는 일반 주장과는 다르다. 이 제한된 invariant 안에서만 ordinal appearance 연산이 정확하다.

Graph 대안은 검토했으나 이번 subset에는 추가하지 않는다. 추후 위 전제가 깨지는 기능을 지원하려면 제3의 authoritative graph source variant를 고려한다. 그 경우 global append trace nodes(appearance1회), ordered per-equation memberships, stable trace IDs를 저장하고 SortedTraces를 파생해야 한다. 기존 AxisLists에 appearance mirror를 덧붙이는 방식은 피한다. 현 r156 Legacy/AxisLists JSON은 이 설계 때문에 바뀌지 않는다.

## 독립 reference와 범위

제품 코드를 import하지 않는 `artifacts/rea/xy-slot-membership-reference-20261019.py`는 실제 List append/insert, source first/Skip(1), successor prepend·마지막 append와 SortedTraces 선택을 따로 모사한다. 결과는 같은 basename JSON에 보관한다. n,m=1..4, 모든 canonical membership과 가능한 global append-order permutation205개, 양 축의 모든 유효한 중간/경계 k 전이1,828개를 검사했다. 삽입672개(신규172·재연결500), 삭제1,156개(제거656·재연결500), 반례0이다. 검사 대상은 min-index binding, membership의 sorted ordinal 순서, appearance 삽입/제거/유지다.

이는 원본 실행 관찰이나 무제한 상태 검증이 아니다. count<=4의 정규화된 모든 입력과 해당 범위의 모든 전이가 닫히므로 그 범위의 임의 길이 sequence를 귀납적으로 다룰 수 있다. 더 큰 count에는 위 네 분기의 일반 귀납과 제품의16개 예산 경계 회귀를 함께 사용한다. simulator가 생략한 visual membership, legend, result 재계산 callback과 native focus 상태를 검증했다고 하지 않는다.

독립 reference 예에서 `p#9`는 새 equation ID, A/B/C/D/N은 각각 기존 또는 새 trace의 **서로 다른 appearance payload**다. 아래 trace 식은 source의 min-index pairing을 적은 것이며 native runtime 캡처가 아니다.

| 이전 source/appearance | 연산 | 이후 source/appearance |
| --- | --- | --- |
| X=[a#1,b#2],Y=[u#3,v#4,w#5,z#6], apps=[A,B,C,D] | X index1 삽입 p#9 | X=[a#1,p#9,b#2], apps=[A,B,C,D]. trace B만(p,v), C/D는(b,w)/(b,z) |
| X=[a#1],Y=[u#2,v#3], apps=[A,B] | X index0 삽입 p#9 | X=[p#9,a#1], apps=[A,B]. A=(p,u),B=(a,v); 신규 style 없음 |
| X=[a#1,b#2,c#3],Y=[u#4,v#5], apps=[A,B,C] | X index1 삽입 p#9, supplied=N | X=[a#1,p#9,b#2,c#3], apps=[A,N,B,C]. 기존 B/C payload는 뒤로 이동 |
| X=[a#1],Y=[u#2], apps=[A] | Y index0 삽입 p#9, supplied=N | Y=[p#9,u#2], apps=[N,A]. model append order가[A,N]이어도 native sorted 순서는[N,A] |
| X=[a#1,b#2,c#3],Y=[u#4,v#5], apps=[A,B,C] | X index1 삭제 | X=[a#1,c#3], apps=[A,C]. removed ID#2와 appearance B 반환 |
| X=[a#1,b#2],Y=[u#3,v#4,w#5,z#6], apps=[A,B,C,D] | X index0 삭제 | X=[b#2], apps 불변. A..D 모두 b에 재연결 |
| 같은2/4 상태 | X 마지막 index1 삭제 | X=[a#1], apps 불변. 제거한 마지막 membership suffix를 a 뒤에 append |
| X=[a#1],Y=[u#2,v#3], apps=[A,B] | X 유일 슬롯 clear | X=[PH#1], apps 불변. ID#1 유지·count0 금지 |

같은 문자열 a가 두 슬롯에 있어도 다른 equation ID면 독립이다. 삽입은 문자열 split/copy/dedup을 하지 않는다. 삭제한 ID는 일반 후속 삽입에서 재사용하지 않으며 기존 source 목록의 ID를 ordinal에 맞춰 다시 번호 매기지 않는다.

## 제안 API와 원자성

`PlotAxis::{X,Y}`와 `insert_axis_equation(axis,index,source,new_appearance: Option<PlotTraceAppearance>) -> Result<PlotAxisEditOutcome,String>`, `remove_axis_equation(axis,index) -> Result<PlotAxisEditOutcome,String>`를 제공한다. outcome은 axis/index/equation_id, change(Inserted/Removed{equation}/Cleared{previous_source}), trace_change(Unchanged/Inserted{index,appearance}/Removed{index,appearance}), migrated를 가진다. trace_count_delta()가 −1/0/+1을 반환한다. clear_axis_equation은 Result<Option<Outcome>,String>이며 기존 PH면 None이다. axis_equation_count와 axis_insertion_requires_trace는 호출자의 사전 조회에 사용한다. n>=m 삽입은 appearance Some을 요구하고 shorter 삽입은 None만 허용한다. 모델이 selector index, palette, 기본 trace type을 추정하거나 selector를 호출하지 않는다. UI는 trace 추가 필요 여부를 먼저 확인하고 provider의 state도 실패 시 복구해야 한다.

clone한 candidate를 사전 검증하고 legacy이면 explicit migration한 뒤 ID 할당·목록·appearance 변경을 수행하고 다시 검증한 후에만 교체한다. invalid axis/index,16개 상한,4096byte source/appearance 예산, ID overflow 또는 잘못된 appearance는 원본 source/allocator/bytes를 바꾸지 않는다. 현재16개는 앱 정책이며 원본 cap이 아니다.

마지막 슬롯은 remove에서 거절하고 별도 clear로 source를 빈 문자열(PH 표현)로 만들되 ID/appearance/count를 보존하는 방식을 우선한다. 이미 빈 PH의 clear는 no-op이다. 빈 문자열을 PH로 사용하는 것은 이 문자열 subset의 표현이며 whitespace 문자열을 임의로 trim해 PH로 취급하지 않는다. 원본 empty-key 제거는 expression과 scaling 모두 empty여야 하므로 UI는 별도 structural predicate를 사용해야 한다. scaling/units가 있는 원본을 문자열 empty만으로 삭제하지 않는다.

기존 Legacy는 구조 편집이 실제로 성공하는 같은 transaction 안에서만 AxisLists로 migrate한다. 기존 NxN pair는 같은 문자열도 각 슬롯에 별도 ID를 주고 inactive legacy fallback은 명시 migration 시에만 버린다. 실패/no-op 때문에 기존 JSON이나 cache SHA가 바뀌지 않는다. 이미 AxisLists인 source는 기존 ID/next allocator를 그대로 유지한다. primary 텍스트 편집·열기·저장만으로 migration하지 않는다.

Undo/Redo는 기존 region/document before/after snapshot을 재사용한다. Redo에서 새 ID나 appearance를 다시 할당하지 않고 저장한 after source를 복원한다. allocator를 포함한 exact source 복원도 검사한다. Undo 후 새 분기가 예전 숫자 ID를 다시 쓸 가능성에는 UI owner generation/IME reset을 함께 사용해야 하며 ID만으로 늦은 이벤트를 허용하지 않는다. pure model은 계산·백그라운드 submit·cache 복원을 실행하지 않는다. source 변화의 signature invalidation, 이전 curve Pending 유지와 Enter 확정은 후속 앱 연결의 계약이다.

## 구현 전후 검증과 남은 경계

순수 모델 회귀는 독립 reference의 모든 n/m/k, 동일 문자열·중간 ID 이동, allocator overflow·예산 실패 atomicity, 마지막 PH, legacy migration, 삽입→shorter split→삭제→Undo/Redo exact snapshot sequence를 다룬다. appearance를 trace 이름 A/B/C로 구분해 잘못된 재생성·복사를 잡으며 palette 값은 oracle로 쓰지 않는다. r156 Legacy와 AxisLists의 byte/signature golden을 유지한다.

native serialization의 longer-axis stable sort는 확보됐으나 비primary 축·legend 재연결·잘못된/비canonical 원본 graph와 모든 mixed-edit history를 기존 subset에 포함하지 않는다. 그런 입력의 identity 보존을 검증하지 못하면 Err/readonly로 남긴다. 구조 source 변경 직후 original curve 표시·Undo 결과 callback·Enter 평가의 전체 UI timing은0157이 확정한 범위와 미확정 범위를 구분해 앱 단계에서 검증해야 한다.

### 순수 모델 구현 결과

parent가 최소 invariant를 승인한 후 새 `crates/mathcraft-document/src/plot_axis_authoring.rs`와 `canvas.rs`의 module/re-export만 구현했다. 기존 PlotSource wire 및 r156 model serializer를 변경하지 않았다. insert/remove/clear는 기존 source 사전 검증, candidate clone과 explicit migration, candidate 사후 검증 후 교체한다. 유일 슬롯 remove는 Err이고 clear는 ID·count·appearance를 유지한다. 계산기·worker·native runtime 호출은 없다.

새 focused6개가 통과했다. 그 중 독립 Rust CIL-list reference가 Python과 같은205개 초기 상태/1,828전이를 실제 production API 결과와 대조한다. 나머지는 index/appearance option/UTF8 budget/16개 상한/allocator overflow 실패 atomicity, 같은 문자열의 독립 migration ID, 삭제 후 ID 비재사용, 유일 PH/no-op wire, sequence의 source snapshot Undo/Redo exact 복원을 확인한다. 로그는 `artifacts/rea/xy-slot-authoring-model-tests-20261019.log`다. 전체 document174개 PASS/실패0/ignored3, document all-target Clippy `-D warnings` PASS를 확인했다. root가 추가한 exact empty source↔native ml:placeholder 회귀도 포함한다. 로그는 `artifacts/rea/xy-slot-authoring-document-tests-20261019.log`, `xy-slot-authoring-document-clippy-20261019.log`다. 후속 앱 슬롯 wiring이나 원본 partial-curve failure 표시를 완료했다는 의미는 아니다.
