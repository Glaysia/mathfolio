# XY 축 슬롯 모델·빈 자리 저장·곡선 렌더 입력

2026-10-19. r156 다음 작업이다. [0157 원본 슬롯 계약](0157-native-xy-slot-authoring-contract.md)·[0158 순수 모델](0158-xy-slot-model-design.md)·[0159 렌더 비용](0159-xy-render-cost.md)을 연결한다. GUI·스크린샷·원본 runtime을 사용하지 않는다.

원자적 axis slot 모델 API와 exact empty source PH의 native 저장을 추가했다. 원본 Shift+Enter·다중 inline slot 키/포커스 연결·부분 curve 오류 표시는 아직 후속이며 모델 API 존재를 작성 UX 완료로 세지 않는다. 곡선의 immutable raw 좌표와 lazy renderer projection을 분리하여 egui_plot 입력 배열을 매 frame 복제하는 경로를 제거했다. 같은 고정 source의 Windows/peets 전체 검증과 r160 freeze를 완료했다. 아래 성능 수치는 Windows의 headless draw section에 한정한다.

## 구조 source API와 PH 저장

PlotSpec은 PlotAxis::{X,Y}별 삽입·삭제·clear를 clone한 candidate에 적용하고 사전/사후 검증을 통과한 뒤 교체한다. 기존 Legacy/AxisLists wire를 바꾸지 않는다. 실제 구조 편집 성공 때만 명시적으로 migrate하며 중복 문자열도 독립 equation ID를 갖는다. 삭제/삽입의 appearance 이동은 [0158](0158-xy-slot-model-design.md)의 canonical membership 귀납 및 독립 CIL reference simulator의205상태/1,828전이와 대조한다. 새 trace 서식은 호출자가 제공하며 모델이 원본 selector state를 추정하지 않는다. 단일 마지막 슬롯 제거는 거절하고 clear는 identity를 유지한다. no-op/실패는 migration·allocator도 바꾸지 않는다.

정확히 빈 문자열만 이 plain source subset의 PH로 표현한다. native writer는 ml:placeholder로 저장하고 reader는 식의 root PH를 빈 문자열로 복원한다. 임의 공백을 PH로 바꾸지 않으며 PH의 알 수 없는 속성/본문·미지원 partial 구조를 조용히 버리지 않는다. source-only PH native를 여는 것은 평가 지시가 아니다.

새 앱 probe는 pure API 삽입→PH native 저장/계산0 재열기→식 채우기/두 curve 계산→앞 식 삭제/남은2*x계산→native/PDF→마지막 식 clear/native 계산0 재열기를 확인한다. keyboard UI authoring 테스트가 아니며 로그에도 ui_shortcut=not_yet_wired를 적는다. 이전 Sync curve를 source가바뀐PH에 다시 붙이지 않는 source-signature 경계를 검사한다.

## 렌더 입력 좌표의 수명

CurvePoints는 raw [f64;2] 배열을 immutable Arc로 소유한다. app의 첫 trace 호환 alias도 같은 저장소를 공유한다. egui_plot용 PlotPoint 배열은 그 curve가 처음 그려질 때 한 번 준비하고 Line/Points가 빌려 쓴다. marker frequency1은 같은 전체 slice를 사용하며 sparse marker는 마지막 frequency 한 개만 cache한다. plot.show 바깥의 고정 크기 Arc 목록이 한 frame의 이전 sparse reader 수명을 보장한다. unsafe 배열 cast나 owner ID만으로 좌표를 재사용하는 경로는 없다.

raw 좌표·순서·스타일·JSON 숫자 표기는 유지한다. renderer projection은 JSON/source signature에 넣지 않는다. NaN이 포함된 배열에는 Arc pointer equality 빠른 경로를 적용하지 않아 이전 Vec의 비반사적 float equality를 유지하고 −0 bit도 보존한다. 계산·source commit·native cache 소유권은 바꾸지 않는다. prepared 배열을 계속 보관하므로 첫 준비와 retained payload 비용이 생기며 frame 전체 allocation/CPU·GPU 비용이0이 되었다는 뜻은 아니다.

회귀는 lazy reuse·sparse frequency 교체와 살아 있는 reader·NaN/±0/legacy JSON·raw source 교체를 확인한다. 실제 app16trace의 warm/hover/Text 이후 문서·curve·저장 cache·Undo·Pending·background generation 불변과 계산0도 별도로 검사한다. benchmark의 draw-only 시간과 parent app 불변 검증을 구분한다.

## 비교 측정과 한계

[0159](0159-xy-render-cost.md)의 고정 baseline/candidate와 별도 allocation 실행 파일4개,20 workload·5 timing pair·2 allocation run을 재검증했다. 2,000점×16trace idle draw의 median은0.3599→0.2815ms, frame당 draw 요청 allocation은795,234→283,242bytes였다. 모든 geometry fingerprint/finite mesh/source와 saved JSON 불변 검사가 통과했다. r160 동결 당시 candidate의3개 source hash가 제품과 같고 benchmark summary SHA는 `3f73f2dc70865eb39d66534d983bc7b9f1c5190a65cb9edfd011ffe6eb040726`이다.

marker가많은 case의 tessellation은여전히약40ms이며 작은 marker case는오히려느려졌다. 2,000×16line-only의 raw+준비 좌표 payload는480,000bytes늘어난다. cold draw·메모리·전체 GPU 반응성의 다른 비용을 숨기지 않으며 자세한 전 case와 한계는0159를 따른다. Linux의 제품 correctness와 Windows timing은 구분한다.

## r160 같은 source 검증

Windows workspace1306+backend2, peets1308이 통과했다. 실패0/ignored68, fmt·workspace/backend Clippy·build·reference38saved/38input·Windows release를 확인했다. 양 OS 합산46MCDX/230XML parts XSD 오류0,40PDF 이미지 객체0과 fitz/pypdf 두 reader 추출을 확인했다. 슬롯 pure API와 PH native의 모델 경로를 포함하며 GUI/Shift+Enter 작성 완료나40% 달성으로 확대하지 않는다.

archive `artifacts/rea/mathcraft-plot-authoring-render-20261019-r160.tar`는12,042,240bytes SHA `bc71571f8e9b8454062dd58f2911fa1467a11da68cf9a045e0432edc45e6a8e0`, manifest SHA `48e6456f5b9dc4740dde86667f568c3b1b8305942cf637fb0e9bdf0640418010`,422files/199product Rust다. peets 경로 `/home/peets/Projects/mathcraft-plot-authoring-render-20261019-r160`에서 모든 source hash를 검증했다. 회수 evidence2,129,920bytes SHA `79dee49cd9e0bd9811cf00891c99a8c1fe1f985f275fbe6f1ab68449424ba4f7`는 양끝 같다. `artifacts/rea/plot-authoring-render-integration-20261019-r160/validation.json`은 최종 current product=archive를 확인했다. archive의 준비 문서와 후속 최종 문서는 구분하며 제품 source는 고정했다.

release23,517,696bytes SHA `8d996b21c6759ffaed00c41221781f79cd340bb2c400e0593cc3280a389508ab`, CPU harness24,885,760bytes SHA `c4d6a95986b59236162f9fc336b887d8b07cd83001cc7d6ea3b00e6b4fbad5f8`다. release는r156보다8,704bytes늘었다. 다음은 [0163](0163-native-empty-and-partial-plot-contract.md)의 빈 도표와 trace별 결과 소유권이며 source/계산/저장/표시를 함께 연결한다.
