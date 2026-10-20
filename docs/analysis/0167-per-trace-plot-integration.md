# 빈 XY·곡선별 결과·native 저장 통합

2026-10-19. r160 다음 작업이다. [0163 원본 빈 도표/부분 오류](0163-native-empty-and-partial-plot-contract.md), [0164 legend source](0164-native-xy-legend-source.md), [0165 오류 캐시](0165-native-xy-error-cache-contract.md), [0166 앱 수명](0166-per-trace-result-lifecycle.md)을 연결한다. GUI·스크린샷·Mathcad/CLR 실행을 사용하지 않는다.

새 XY는 빈 두 축에서 시작하고 실제 키 입력은 draft에만 남는다. Enter로 한 번 확정하고 수동 계산을 요청하면 curve를 얻는다. 곡선별 Ready/Failed/Pending/Incomplete/Unavailable을 수집·저장·표시하며 한 오류가 다른 성공 곡선을 없애지 않는다. 실패 후 예전 곡선이 native 저장에서 다시 나타나는 경로를 분리했다. native의 빈 Pending은 원본 엔진 오류를 새로 생성한 결과가 아니다.

headless 앱 probe는 실제 명령을 사용하여 Ready 세 곡선→중간 식 오류 편집/전체 Pending→계산/중간 Failed→own cold reopen→native Ready/Pending-empty/Ready→native copy 원문 유지→own mixed cold reopen→식 수정/좌표 있는 Pending 두 개와 좌표 없는 Pending 한 개→재계산/세 곡선 회복을 확인한다. 각 cold open 계산은0이고 Failed/회복 PDF를 생성한다.

별도 blank probe는 새 리본 삽입의 두 PH/한 trace/curve 없음·Bottom/Right source, own/native cold open 계산0, 실제 Text/Tab/Text/Enter의 draft 불변과 단일 확정, 2*x 계산·native 재열기·벡터 PDF를 검사한다. legend fixture는 absent/explicit 위치·legendWidth의 정확한 finite double 저장을 검사한다. 위치 source 보존은 오른쪽 legend layout 완료와 구분한다.

같은 r167 고정 source를 Windows workspace1335+backend2, peets1337이 통과했다. 실패0/ignored71, fmt·workspace/backend Clippy·build·reference38saved/38input·Windows release를 확인했다. 양 OS 합산62MCDX/310XML parts의 원본 XSD 오류0,46PDF 이미지 객체0과 fitz/pypdf 두 reader 추출을 확인했다. validator는 세 resultRef의 ordinal별 Pending/Sync 상태·곡선 유무·각 좌표의2x/4x/3x 관계, 빈 source PH, legend 위치/finite width bit도 대조한다. 기능 목록의 행 수를 완성률로 환산하지 않으며 40% 달성으로 판정하지 않는다.

남은 공백은 다중 inline 슬롯 삽입/삭제·단위 scaling·원본 표본 domain·legend 실배치·graph/region geometry·원본 오류 결과 serialization·모든 plot type이다. r160의 제한된 headless 성능 수치를 새 전체 앱 반응성 수치로 재사용하지 않는다.


## r167 고정 산출물

archive `artifacts/rea/mathcraft-plot-outcome-20261019-r167.tar`는12,236,800bytes SHA `f426652865fa1b83c6362e20a382c89ae11305e29eeec69184850c96a0e7a5e6`, manifest SHA `03f7aa30367125e6dd393c46ad8d709e86fb4580f73fbd6784d73e9caa303946`,436files/206product Rust다. peets `/home/peets/Projects/mathcraft-plot-outcome-20261019-r167`에서 모든 source hash를 검증했다. 회수 evidence2,385,920bytes SHA `819059a1c4e312ce2eb14f04c22195cc0b6613210016a2b86675de2250b9f2dd`는 양끝 같다. `artifacts/rea/plot-outcome-integration-20261019-r167/validation.json`은 최종 시점의 current product=archive를 확인했다. 이후 작업 트리는 별도 revision으로 검증한다.

release23,642,624bytes SHA `b328371eb0a7e17eb9a30a60478c89d5b91604c2474573676861d8d10e58ae86`, headless harness25,265,664bytes SHA `8ba78c271fe35e036537455d00315549c65554d27825d7a11a8dd77c1306b08c`다. 앱 실행/GPU 반응성을 이 CPU 검증으로 인증하지 않는다. r160 대비 release는124,928bytes 증가했다.
