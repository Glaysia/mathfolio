# Native XY source·곡선 결과·편집·벡터 출력 연결

2026-10-19. 사용자 평가15%에서40% 목표로 진행하는 중간 묶음이다. [0146 source](0146-native-xy-plot-export-contract.md), [0148 결과](0148-native-xy-result-contract.md), [0149 domain/commit](0149-native-xy-domain-and-commit-contract.md)의 설치본 정적 CIL/BAML/XSD 근거를 사용한다. GUI·스크린샷·원본 Mathcad/CLR/COM을 실행하지 않았다.

## 원본 저장 구조와 앱 연결

지원되는 XY source는 원본 worksheet50 plot/xyPlot, N개 trace와 각 축 N개 equation, 식과 unit scaling의 별도 math, placeholder domain, graph-size와 선 서식을 저장한다. 같은 x식을 여러 trace에서 사용해도 ordinal pairing을 유지한다. 새 trace resultRef는 모든 worksheet part의 참조·기존 결과 ID를 포함한 전역 allocator로 예약한다. XML Schema int의 공백·부호·선행0을 동일한 ID로 취급하며 원본 lexical bytes는 수정이 필요한 경우 외에는 보존한다.

새·편집된 지원 source는 MCDX로 내보내고 다시 editable PlotSpec으로 읽는다. unknown 속성/자식·정밀도 손실·표현 불가 서식은 원본 raw/Opaque로 보존하거나 atomic export error로 거절한다. 현재는1–16 trace, 무차원 실수 선 그래프와 기본 축 표시만 지원한다. trace label, marker, 비기본 축, 단위 scaling, unequal x/y equation 목록은 미완료다. 전체 영역 높이와 원본 Graph 실제 높이를 구분하며 Mathcraft의 기존50 DIP allowance는 원본 상수가 아니다.

곡선 결과는 일반 ml:result wrapper 없이 resultData의 direct ml:Trace2dResult로 저장한다. RangeInfo/ResultInfo와 Columns/Numeric 각1 vector의 flat JSON x/y를 사용한다. 원본 현재 serializer의 flat 형식을 따르며 오래된 XSD comment의 nested 배열을 따르지 않는다. finite·unitless·동일 길이·trace당2000점·현재 source signature·스타일·상태가 확인된 실제 앱 계산만 새 Synchronized 결과가 된다. saved cache XML/점은 evaluator binding이나 코드로 실행하지 않는다.

native cold open은 저장된 점/스타일을 계산 없이 복원한다. 원본 유래 cache는 inert marker를 유지하고 그대로 재저장한다. 한 그래프의 일부 trace만 Pending이어도 다른 trace의 Synchronized 상태를 부모의 Pending으로 덮지 않는다. 실제 재계산은 이 marker를 제거하고 새로운 현재 점만 기록한다. 전체 source/context 변경에 대한 r145의 보수적 Pending 전환은 유지한다.

## 모호한 캐시와 지원 경계

worksheet owner ID·result ID·resultRef의 중복/숫자 alias를 거절한다. actual namespace의 resultData 전체 구조를 part당 한 번 검사하고 curve codec을 trace당 한 번 적용한다. 추가 retained index에 point vector나 DOM을 복제하지 않는다. missing ref/cache 또는 정상적인 빈 Pending row는 계산 대기이며 결과를 발명하지 않는다. unknown resultData sibling·진단·단위·Rows/multi-vector·비유한 점·초과 크기는 지원 필요 진단을 남긴다. 해당 native graph가 빠진 상태로 정상 PDF를 생성하지 않는다. own 저장/재열기에도 진단을 복구하며 실제 계산으로 현재 곡선을 만들면 이전 decode 오류를 해제한다.

원본의 수동 x domain은 실제 샘플 구간에 영향을 주지만 현재 PlotSpec.range는 표시 범위다. 이를 원본 domain으로 임의 직렬화하지 않는다. native curve의 RangeInfo/ResultInfo 원문은 보존하되 앱 화면의 모든 viewport 상태를 재현한다는 뜻은 아니다. source 변경 후 새 resultRef에 이전 trace payload를 옮기는 기능과 모든 native curve/단위·3D·Chart Component는 미완료다. 원본 runtime의 새 파일 열기 수용성은 별도 미검증이다.

## 통합 검사

실제 앱의 gain0:=2와 single/multi XY2개·3 traces·1500점 계산→MCDX→cold open에서 실행0/Pending0, 원래 점/색/선 서식 일치를 검사한다. own save/reopen→native copy의 result.xml은 bytes가 같다. 실제 편집 draft는 commit 전 source/점을 바꾸지 않고, 수동 계산 상태에서 commit 후 Pending, 재계산 후 y=2x와 새 native cache 재열기를 검사한다. vector PDF는 계산 없이 생성한다.

추가 회귀는 malformed/style mismatch/Pending fresh promotion 거절, XML int alias 예약, mixed trace 상태의 exact 재저장, 미지원 Rows의 표시·print block·own 저장·재계산 복구, source-only Pending의 editable cold open과 명령 계산을 확인한다. 원본 제품 자료가 아닌 독립 fixture이며 XSD·두 PDF reader 검증과 Windows/peets의 같은 소스 통합 검사는 r152 고정 후 아래에 기록한다. 테스트 수를40% 완성으로 환산하지 않는다.

첫 r150 gate는 Area 저장 결과 독립 fixture의 비정상 result ID(r0/r1/r2)를 정수 결합 경로가 거절하여 기존5검사가 실패했다. 원본 XSD int 계약에 맞게 fixture의 세 ID/참조를10/11/12로 수정했다. 기대값/검사 내용은 유지했으며 Area focused7개가 다시 통과했다. 실패 archive/log는 남기고 새 r152 소스를 고정한다.

## r152 통합 검증

Windows workspace1251+backend2, peets workspace+backend1253 통과, 양 OS 실패0/ignored65다. fmt·workspace/backend Clippy·build·reference38saved/38input와 Windows release도 통과했다. 양 OS 합산22MCDX/110XML part XSD 오류0,28PDF 이미지0·PyMuPDF/pypdf 독립 추출을 확인했다. 곡선 데이터는 각 파일의 실제 active trace resultRef로 찾아3traces×500점을 확인하며, 편집 후 남는 orphan Pending 원문을 active 결과로 세지 않는다. GUI·원본 runtime·실물 프린터 결과는 아니다.

고정 archive11,704,320bytes SHA256 `d2b634feb245ac896d7ecbc6393da58d737089e027bbff6992e778903a331c26`, manifest SHA `151a037aef31cc687a2627496cfebc2a638ca8ee97332da0d332de0c293388a1`,405파일/192product Rust다. peets 경로 `/home/peets/Projects/mathcraft-native-xy-20261019-r152`에서 archive/manifest/각 파일 bytes를 다시 검증했다. 회수 evidence1,648,640bytes SHA `f97d8e56494d407b4ee3cf55ab4e7173d67e2568a149fbbfa2ea3354aa91ac19`는 양끝 hash가 같다. 최종 manifest 검사는 current product=archive를 확인했다. 검증 record는 `artifacts/rea/native-xy-integration-20261019-r152/validation.json`이며 이 최종 문서 기록은 archive의 준비 상태 기록 뒤에 추가했다.

고정 release23,304,704bytes SHA `29738ad8100abe3bf5c463fef1ed40e730f5cafc20f643628da8380bada40448`, CPU harness24,370,176bytes SHA `02d1c5f7ac8381cfbd69eb29654304d9db1e3e15a1b993d54330e6a13fd8e78b`다. release는 r145보다169,472bytes 증가했다. 이번 source/helper 중복 XML 생성·파싱을 제거했지만 별도의 전체 앱 성능 개선 수치를 주장하지 않는다.

다음 조사 [0151](0151-native-xy-axis-pairing-contract.md)은 unequal axis 목록의 마지막 equation 공유 identity와 Shift+Enter 추가/Enter 계산을 확인한다. N/N guard를 제거하여 문자열만 복제하면 공유 식을 한 번 수정하는 원본 의미를 잃으므로 실제 source model과 편집 경계를 연결하기 전까지 부분 구현으로 유지한다.
