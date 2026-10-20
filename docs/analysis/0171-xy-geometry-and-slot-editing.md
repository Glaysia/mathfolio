# XY 크기 source와 여러 축 식 입력 연결

2026-10-19. [0168 원본 크기 계약](0168-native-xy-graph-region-geometry-contract.md)·[0169 설계](0169-xy-geometry-source-design.md)·[0161 입력 목록](0161-native-xy-expression-layout-contract.md)을 Rust 문서/앱에 연결한다. GUI·스크린샷·원본 실행을 사용하지 않는다.

## 서로 다른 크기의 저장

`PlotSpec.native_geometry: Option<Box<PlotNativeGeometry>>`를 추가했다. None은 old JSON/export adapter이고 Some의 graph_size None은 native graph-size 부재다. Some의 명시 graph_size, requested_width/height, actual_width/height는 독립 f64 source다. old None의 필드 순서·JSON·계산 서명은 바꾸지 않는다. explicit null carrier·중복/unknown 필드·불완전 graph pair를 거절한다. graph/requested는 finite 양수, actual은 finite 비음수이며 unsupported source는 원본 raw 경계에 남긴다.

None과 native graph 부재를 합치거나 actual을 요청 크기로 승격하지 않는다. requested의 안전한 f32 projection만 앱 allocation에 쓰고 요청 부재의 각 축에는 앱 기본420/260을 쓴다. 이 fallback은 원본 WPF layout의 해석 결과가 아니다. 요청 범위120–2000×100–2000 밖은 이번 editable codec에서 지원하지 않는다. graph source는 화면에서 파생한 height−50으로 덮지 않는다. native finite double의 정밀도와 actual −0 bit도 유지한다.

atomic `set_outer_size`는 실제 바뀐 축의 requested만 갱신하며 바뀌지 않은 축의 정밀 source를 유지한다. 크기가 바뀌면 오래된 actual snapshot 두 개를 제거하고 graph source는 그대로 둔다. no-op와 실패는 원래 모델을 유지한다. carrier Some에서 direct width/height assignment로 source와 allocation이 어긋나면 validate가 거절한다. 새 blank만 requested420/260·graph 부재 carrier를 갖고 old Default는 기존 정책을 유지한다.

Native reader/writer는 optional graph element와 wrapper requested/actual attrs를 독립적으로 읽고 쓴다. same_source는 modeled geometry/absence/float bits를 엄격히 비교한다. `matches_retained_source`는 old None 모델에 대해서만 옛 reader의 graph decimal/f32·+50 제약을 재검증하는 별도 adapter다. 그 adapter로 일반 source guard를 완화하거나 old JSON/cache를 자동 migrate하지 않는다. old reader가 무시하던 현재 범위 밖 outer request까지 모든 old archive가 지원된다고 주장하지 않는다.

요청 source carrier를 inline으로 넣으면 모든 CanvasContent variant의 크기가 증가하므로 Box를 선택했다. 이번 빌드에서 PlotSpec/CanvasContent는240bytes, Option<Box>8bytes이며 geometry가 있는 plot만88bytes carrier heap을 가진다. allocator overhead와 clone 비용은 별도다. 이 선택만으로 전체 앱 메모리/속도 개선을 인증하지 않는다.

## 크기 조절과 Undo

DrawResponse는 resize gesture 시작 때 setter 전 전체 PlotSpec을 한 번만 캡처한다. 앱은 첫 실제 delta에서 그 source를 region history로 보관한다. 이전처럼 f32 두 값으로만 되돌리면 사라진 requested 정밀도와 actual snapshot을 복원할 수 없다. 새 history는 식·graph·requested·actual·allocation을 함께 복원하고, 한 drag는 Undo 한 번이다. pointer delta가 실제 크기 변경을 만들지 않으면 history/계산을 만들지 않는다.

동시에 확정할 축 draft가 있으면 최신 outer size를 setter로 적용한 candidate를 검증한 뒤 설치한다. IME preedit는 source에 넣지 않는다. topology/계산 대기/이전 좌표의 수명은0166을 유지한다. 실제 pointer headless 회귀와 별도 controller→own/native/print 회귀를 구분한다.

## 모든 authored expression 슬롯의 입력

AxisLists의 모든 X/Y 식과 Legacy의 각 실제 pair를 독립 TextEdit로 연결한다. AxisLists는 equation ID에 caret/IME를 귀속시켜 문자열이 같거나 순서가 바뀌어도 다른 식의 state를 재사용하지 않는다. source topology가 바뀌면 기존 전체 draft를 새 source 위에 설치하지 않는다. identity가 없는 Legacy는 exact cached source 비교로 세대를 분리하고 snapshot은 변경 때만 복제한다. 매 frame 64bit 문자열 hash를 권한 판정에 쓰지 않는다.

지원하는 expression subset의 내부 Tab 순서는 Y 전체→X 전체이며 ShiftTab은 역순이다. 내부에서 끝→처음으로 wrap하지 않는다. 실제 원본에는 식별 scaling field도 사이에 있으며 그 전체 traversal과 구조 수식 caret는 아직 미완료다. Shift+Enter 새 슬롯·원본 style selector도 이번 범위에 넣지 않는다. 여러 칸의 타이핑은 source/curve/계산/history를 건드리지 않다가 Enter/기존 확정 명령이 aggregate draft를 한 번 반영한다.

화면과 PDF는 식을 개별 row로 표시한다. 실제 row 측정과 X row 소요 높이를 뺀 공간을 graph body에 배정한다. 이는 앱 allocation 정책이며 native graph source나 원본 pixel 계약이 아니다. 최소 body45와 모든 row가 outer 크기에 들어가지 않는 경우의 overflow, 원본 legend 좌우/상하 실배치·adaptive columns·unit scaling은 남아 있다. draw/print는 source 크기를 자동 변경하지 않는다.

기존 immutable curve/prepared 좌표 수명은 유지한다. 다중 field/ID의 bounded 임시 Vec 할당과 새 row 측정이 있으므로 r160의 고정 timing/allocation 수치를 현재 제품의 수치로 재사용하지 않는다. 실제 검증은 [0172](0172-xy-geometry-slot-integration.md)에 기록한다.
