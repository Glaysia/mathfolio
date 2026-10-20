# Native 결과와 페이지 나누기 문서 흐름

2026-10-19. 사용자 평가15%에서40% 목표로 진행하는 중간 묶음이다. 원본 [0141 페이지/공간 명령](0141-page-break-and-space-contract.md), [0143 결과 저장](0143-native-result-persistence.md), [0144 앱 연결](0144-page-break-app.md)의 정적 CIL/BAML/XSD 근거를 사용한다. GUI·스크린샷·원본 Mathcad/CLR/COM은 실행하지 않았다.

## 저장 결과의 수명

이전 exporter는 매번 원본 계산 결과·오류·경고 payload를 지웠다. 이제 전체 ordered source(Area는 부모 위치의 local ordered block)와 package context가 같을 때만 원본 raw resultData와 상태를 유지한다. 개별 변경 없는 행만으로 downstream Sync를 주장하지 않는다. 계산 원문·순서·disable 등 semantic source가 바뀌거나 context가 바뀌면 전체 기존 상태를 Pending으로 전환하되 원본 값·오류·경고·tag는 보존한다. 원본 Pending EngineResult의 copy/status 전이를 따른다. 결과/참조 ID가 모호하면 Sync 유지와 fresh promotion을 거절한다.

Context는 worksheet와 실제 result10 resultsList를 제외한 모든 part의 이름·길이·SHA를 비교한다. 알려진 직접 숫자/기본 글꼴 서식은 계산 원문에서 제외하지만 unknown metadata와 외부 flowdoc/settings/binaries는 유지한다. 현재는 plain text/page part의 변경에도 보수적으로 Pending을 만든다. 계층과 source ID를 확인할 수 없는 원문에는 동일성 성공을 반환하지 않는다.

새 결과는 현재 source signature·owner/Area path·최종 resultRef·Synchronized 상태를 확인한 뒤 finite 무차원 실수/복소수/행렬의 literal만 resultData에 쓴다. ml:result wrapper와 column-major matrix를 사용한다. 명시 단위변환은 이미 변환된 display_value를 기록하며 원본처럼 단위를 중복하지 않는다. malformed/unsupported fresh value는 예전 Sync를 남기지 않고 Pending으로 둔다. 이 값은 재열기/재서식/인쇄용이며 evaluator binding 복원에 사용하지 않는다.

실제 앱 계산→MCDX→cold native open→vector PDF에서 7.125,2+3i,2×3 행렬[1..6],0.001s→1ms의 네 query가 실행0/Pending0으로 복원된다. 새 Area의 definition/query도 재계산 전에 저장 표현을 복원한다. 원본 raw structured 진단의 source/context 유지와 새 현지화 문자열에서 native code를 재구성하는 것은 구분한다. 새 unitMonomial/range/symbolic/plot/structured diagnostics writer는 미완료다. Pending 오류/경고의 원본 화면 표시 정책은 추가 근거가 필요하다.

## 페이지 나누기와 출력

blank root Ctrl+Enter와 원본 Document Spacing 명령은 전체 committed live bounds를 먼저 검증하고 suffix 이동·새 pageBreak·입력점 이동을 한 Undo로 적용한다. 새 소스는 empty pageBreak payload·height2이며 native 전체 ID와 Canvas authoritative 좌표를 저장한다. selected/active Ctrl+Enter는 기존 편집 명령을 유지한다. RemoveSpaceAbove/Below는 현재 검증한 단일 인접 break 구간만 지원하고 일반 공백 편집으로 확대하지 않는다. protected/unknown geometry·PageWide 교차는 변경/이력 없이 거절한다.

pageBreak는 원본처럼 일반 수식 선택/편집 대상에서 분리하고 passive 편집선으로 표시한다. 원본 별도 ItemsControl의 RenderTransform과 절대 canvas mapping은 구분하며 -2*collectionIndex를 절대 좌표에 다시 적용하지 않는다. 전체 WPF panel/global marker mapping·원본 brush색 일치는 미확정이다. 인쇄 snapshot은 원본의 WhiteboardItems-only 경로를 따라 편집선/placeholder를 그리지 않는다. 뒤쪽 좌표가 이미 이동했으므로 출력에서 가짜 추가 offset을 만들지 않는다. 끝의 break도 다음 빈 페이지를 포함한다. 계산용 원본 body-height f64를 page/print 경계에 유지하고 f32 반올림을 native 좌표에 저장하지 않는다.

## 큰 Area 비용과 검증 경계

[0142](0142-area-lookup-cost.md)의 source-bound child key index는 warm frame의 반복 linear lookup 두 곳을 바꾼다. 2048math UI+tessellation 중앙값14.224→6.488ms, mixed13.554→6.149ms였다. 작은 mixed16과 collapsed1024는 소폭 악화했고 메모리 key/capacity 모델도 함께 기록한다. 전체 app/GPU/입력 지연이 절반이라는 주장이 아니다. native 결과 writer의 owner/row/path 바인딩도 새 반복 N² lookup을 추가하지 않도록 borrowed index를 사용한다.

r145 소스 고정·Windows/peets gate·생성 native XSD·독립 PDF 검증 준비 중이다. 최종 숫자와 파일 정체성은 artifacts/rea/document-flow-integration-20261019/validation.json 및 아래 후속 기록에 남긴다. 이 기록과 검사 개수를40% 완성으로 환산하지 않는다.

## r145 최종 통합 검증

Windows workspace1217+backend2, peets workspace+backend1219 통과, 양 OS 실패0/ignored63이다. 두 OS의 fmt·workspace all-target Clippy·backend Clippy·build, reference38saved/38input가 통과했다. Windows release도 생성했다. 새 native 결과·페이지 나누기·기존 Area/행렬 fixture와 원본1015/1016의 cold vector 출력까지 확인했다. 양 OS 합산16MCDX의 worksheet/header/footer/presentation/result80 XML parts XSD 오류0,22PDF의 PyMuPDF/pypdf 독립 추출·이미지0을 확인했다. GUI·프린터 실기나 원본 Mathcad의 새 파일 열기를 수행한 결과와 구분한다.

Archive11,530,240bytes SHA256 `c52adb70ff0dfe5e3e8ca7a319414a7d21ad1df498f6b3db43e0a42ff60e2443`, source manifest SHA `a5694dab79e4e9f07c12063e052ebf5613a2183987a11e49d7dc69cbdcdc7368`,394파일/185product Rust다. peets 경로는 `/home/peets/Projects/mathcraft-document-flow-20261019-r145`이며 source/manifest/각 파일 hash를 다시 확인했다. 회수한 evidence931,840bytes SHA `6922a6cd5aeac35db5c8aba21df5552b15f3c3c5c495171ee6ade1853f0f2625`의 양끝 정체성도 같다. 최종 검사 시 current product와 archive의 bytes가 같았다. archive의 문서에는 검증 준비 상태가 들어 있으며 이 최종 기록과 이후 조사 문서 갱신은 그 고정본의 제품 소스를 바꾸지 않는다.

Frozen release23,135,232bytes SHA `810533255b5aff47368d38f458cc1cd04d8031285b969f53459edbdbeceeb00e`, CPU harness24,016,384bytes SHA `4bf4099c0c1f5e27fe02704b5388dd19f5ce0a43b7c233a03f6eee4e3c648cdd`다. release는 r140보다280,576bytes 증가했다. 성능 측정은0142의 bounded CPU 경로이며 전반 용량 감소·GPU 체감 개선을 주장하지 않는다.

이전 source-export 회귀의 'Pending이면 값 삭제' 기대는 원본 Pending 계약에 맞게 수정했고, 누락된 Area child resultRef에도 신규 Pending row를 할당한다. 모든 XML part의 Math resultRef를 예약/계수하므로 저장 resultData가 없는 SpecTable column과 header/footer reference도 새 ID 공간에서 제외된다. 현재 값으로 직렬화할 수 없는 새 diagnostic은 이전 Sync 값을 남기지 않는다. 원본 diagnostics의 정확한 native code/ID를 현지화 문자열로 발명하지 않는다.
