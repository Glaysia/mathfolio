# XY 물리량·배율·결과 단위 연결

2026-10-19. [0170 원본 계약](0170-native-xy-unit-scaling-contract.md), [0174 source 설계](0174-xy-scaling-source-design.md), [0175 계산 설계](0175-xy-unit-evaluation-design.md)의 후속 구현이다. 원본 GUI/CLR/COM 실행 없이 설치 CIL·BAML·도움말·XSD와 독립 Rust 검사를 사용한다. 전체 원본 일치 또는40% 달성을 뜻하지 않는다. 전체 고정 소스 검증은 [0178 통합](0178-xy-units-integration.md)에서 별도로 기록한다.

소스는 각 authored equation의 `PlotScaling` typed tree 하나가 권위다. PH/missing은 자동, 명시 source는 UNIT/VARIABLE/CONSTANT labels, contextual, literal subscript를 보존한다. 동일 X 또는 Y를 공유하는 pair에 scale source 복제본을 저장하지 않는다. 기존 None JSON·계산 서명·unitless v1/v2 wire는 유지한다. 두 번째 Math가 source scaling이며 result rangeUnits/resultUnits의 unitMonomial과 다르다.

계산은 compat 환경의 물리 Quantity를 보존한다. X를 pointwise Y에 대입할 때 canonical 숫자와 강제 UNIT 기본 단위의 typed tree를 만든다. worksheet 변수 m/s 등으로 표본 차원이 바뀌지 않는다. 모든 표본의 실수/유한성·차원을 확인한 뒤 사용자 배율로 좌표를 나눈다. 예를 들어 X=[1,2,3] m, X배율=in, Y=x², Y배율=m²는 X=[1,2,3]/0.0254, Y=[1,4,9]다. X배율=0.1 m과 dimensionless0.1은 좌표값은 같아도 전자의 잔여 단위는 없고 후자는 meter다.

첫 자동 단위 정책은 독립 엔진의 정수 SI 기본 차원 곱이다. native canonical meter/kilogram/second/ampere/kelvin/mole/candela는 BAML 양성 근거의 이름을 쓰며 glyph m/kg 등을 native @unit으로 쓰지 않는다. 원본 자동 derived-unit 선택, 전체 unit-system 설정, affine 변환을 재현했다고 주장하지 않는다. 분수 차원 자동 단위는 명시 compatible scaling이 없으면 지원 오류다. scaling은 유한 양수 scalar만 허용하고 complex/zero/negative/incompatible은 해당 곡선 Failed다. 물리 단위를 붙인 미정의 implicit X는 아직 미지원이며 explicit physical range/vector를 요구한다. 기존 dimensionless implicit −10..10/500점은 유지한다. 자동 또는 공유 식의 once-evaluation은 원본 근거 미완료라 memoize하지 않는다.

결과 metadata는 immutable Arc pair 안의 X/Y별 `PlotAxisPresentation`으로 보관한다. Calculated는 authored source snapshot, 실제 factor, physical dimension과 residual units를 분리한다. Native는 source snapshot과 원래 residual units만 보관하며 배율을 추정하거나 실행하지 않는다. metadata가 있으면 저장 wire v3의 ordered state wrapper를 쓰고 각 curve의 x/y_presentation 키는 null도 명시한다. 구형 v1/v2는 새 키를 받지 않는다. Ready는 현재 label/style/typed scaling과 정확히 맞아야 복원하며 Pending.previous는 과거 source/units를 그대로 보존할 수 있다. source/context 변경 직후 Ready를 Pending으로 옮기므로 이전 좌표를 새 배율의 최신 결과로 저장하지 않는다.

native decode는 기존 unitless finite codec을 그대로 두고 bounded units codec을 추가했다. 읽을 수 있는 unknown canonical 이름과 system/power lexical 의미는 inert metadata로 보존한다. native copy는 원래 raw payload를 유지하고 계산하지 않는다. 새 계산의 native export는 좌표와 residual units만 쓰며 사용자 scaling에서 단위를 추측하지 않는다. Failed/Incomplete는 fake native EngineError를 만들지 않고 기존 Pending clear 정책을 따른다. 원본 live engine clear 수명 계약은 미확정이다.

입력은 기존 aggregate draft에 retained expression-only Editor를 붙인다. Tab 순서는 Y식/Y배율을 각 줄마다 지나 X식/X배율로 진행하고 밖에서 wrap하지 않는다. Enter/저장/F5/포커스 이탈이 확정 경계이며 IME composition과 invalid scaling은 확정을 막는다. 입력 중 document/results/history/generation은 그대로다. scaling에서 새 nullable identifier의 UNIT default는 원본 계약의 bounded 산술/Group/Index/Row/Column 범위만 연결하고 imported NativeIdentifier·explicit label·suffix는 그대로 둔다. Call/binder repair 전체는 미완료다. 한 입력과 default label은 같은 local Undo group이다.

인쇄는 authored scale와 readonly residual unit을 구조 수식으로 그린다. runtime 입력/print가 automatic units를 authored source로 승격하지 않는다. canonical 표시 이름은 설치 descriptor에서 양성인 작은 맵만 쓰고 degree의 두 alias 우선순위는 추측하지 않는다. unknown 이름은 원문 표시한다. caches는 Arc로 보유해 idle에 typed tree를 복사하지 않는다. 잔여 원본 영역은 전체 EquationControl traversal/수식 source, 전체 단위 체계·affine/complex 정책·legend/adaptive layout·구조 슬롯 키·다른 plot 종류와 GPU 실제 반응성이다.
