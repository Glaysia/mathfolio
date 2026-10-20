# XY 독립 크기·여러 축 식 입력 통합

2026-10-19. r167 다음 작업이다. [0171 구현](0171-xy-geometry-and-slot-editing.md)에 graph/requested/actual source 분리, old model adapter, 전체 source를 복원하는 resize history, 여러 authored expression 입력을 연결했다. [0170 단위 분석](0170-native-xy-unit-scaling-contract.md)은 다음 단계의 근거이며 단위 지원 구현 완료가 아니다.

앱 geometry probe는 outer600.00000000001×400.00000000002, graph321.00000000001×217.125, actual598.75×399.5를 own/native cold reopen 계산0으로 복원한다. native copy의 worksheet/result XML bytes와 print 전후 source도 같아야 한다. resize는 outer680×440만 바꾸고 actual을 지우며 graph는 유지한다. 수동 Pending의 이전 2*x curve는 새 native ref에 Pending으로 저장하고 Undo/Redo는 source 전체를 복원한다. 별도 실제 pointer 회귀가 setter 이전 metadata 캡처와 한 drag/한 Undo를 확인한다.

여러 슬롯 앱 probe는 shared X 두 개/Y 세 개를 만든 뒤 실제 Tab/Text로 Y2=9*x와 X1=6*x를 수정한다. Enter 전 source/curve/history/background generation은 불변이며 확정 한 번, Undo/Redo, Pending→계산→세 pair의3/4÷6/9÷6 비율, own/native 저장·cold reopen 계산0과 PDF를 확인한다. 공유 식의 권위를 pair별 복제하지 않고 ID를 유지한다.

old None 모델+retained native archive+own cache 부재도 별도로 검사한다. 재열기에서 source를 자동 migrate하지 않고 지원 curve를 복원하며 native XML/result는 그대로 복사한다. 해당 plot 식이 archive보다 새롭다면 그 archive의 curve를 붙이지 않고 Pending으로 남긴다. strict cache inspection 진단과 legitimate own-format edit의 표시 부재를 구분한다. 이 회귀를 임의 외부 편집된 own 문서의 모든 upstream context 인증으로 확대하지 않는다.

문서 회귀는 precision/absence·partial requested·actual ±0·atomic setter/no-op·old JSON/SHA·UTF16/raw source·legacy adapter negatives와 wrapper patch를 검사한다. source geometry fixture는 explicit와 graph 부재 두 plot을 만든다.

r172의 같은 고정 source를 Windows workspace1355+backend2, peets1357이 통과했다. 실패0/ignored74, fmt·workspace/backend Clippy·build·reference38saved/38input·Windows release를 확인했다. 양 OS 합산72MCDX/360XML parts 원본 XSD 오류0,50PDF 이미지 객체0·fitz/pypdf 두 reader 추출을 확인했다. validator는 requested/graph/actual 독립값·actual −0 bit·graph 부재·resize actual 제거·X2/Y3 pairing과 곡선 비율도 검사한다. 목록의 세부 행 상태는 기존 부분 구현 범위의 확장이므로 올리지 않으며 40% 달성으로 환산하지 않는다.

남은 범위는 unit/scaling source와 결과 단위, Shift+Enter/slot 구조 입력, legend 실배치·adaptive columns·큰 목록 overflow, WPF 최종 크기 수렴, 전체 original error serialization와 기타 plot type이다. GUI/GPU 반응성은 이 headless 검증만으로 인증하지 않는다.


## r172 고정 산출물

archive `artifacts/rea/mathcraft-xy-geometry-slots-20261019-r172.tar`는12,359,680bytes SHA `15663f465c7e11d11712a0bac43193090ecf7a537148f7f99efb2c5a0866ede9`, manifest SHA `927aa19bea8d74b9553e4c1450b94983c7343cbbaeb7e9356f0f0cfad99e5f7f`,444files/210product Rust다. peets `/home/peets/Projects/mathcraft-xy-geometry-slots-20261019-r172`에서 모든 source hash를 검증했다. 회수 evidence2,560,000bytes SHA `d8d883c3ffd2235b764450d4964dd73d4c80a52f8b96bc38873da092f80b0a39`는 양끝 같다. `artifacts/rea/xy-geometry-slots-integration-20261019-r172/validation.json`은 완료 시점 current product=archive를 확인했다. 이후 제품 수정은 별도 revision으로 검증한다.

release23,720,448bytes SHA `e1e1696bdfb15ca6ff466a51dc5d2f5f7edadee3cd548488ab7c6e8ceb530945`, headless harness25,530,880bytes SHA `f418aa2ada7f6b47a6bfbc1e7e57124c3d5108daab279f71fb62f382512acecc`다. release는r167보다77,824bytes 증가했다. 현재 frame timing을 새로 측정한 자료는 아니며 [0173](0173-xy-marker-tessellation-design.md)의 남은 marker tessellation 비용도 설계/이전 측정과 구분한다.
