# 빈 Area와 내부 수식의 작성 흐름

2026-10-19. 사용자 평가15%에서40% 목표로 진행하는 중간 묶음이다. [0138](0138-area-authoring-contract.md)·[0139](0139-area-enter-and-ribbon-contract.md)의 설치 CIL/BAML/스키마 근거를 사용하며 GUI·스크린샷·원본 런타임은 실행하지 않았다.

## 작성과 확정

Ctrl+Shift+A와 Document→Regions→Area는 빈 워크시트 입력점에서 빈 Area를 만든다. 일반 수식이 선택되거나 편집 중인 문맥과 중첩 Area 저작을 거절한다. 선택 수식을 새 Area로 감싸는 동작은 원본 명령이 아니므로 만들지 않는다. 기본 내부 입력점은19.2DIP이고 새 영역의 명시적 owning height는 major grid 한 칸이다. retained AreaHeight는0(default)이며 실제 측정 전 actualHeight를 만들어 저장하지 않는다.

첫 Text/IME 확정은 실제 자식 PH를 source에 추가하고 같은 입력을 자식 editor의 draft에 전달한다. Preedit만으로는 자식·계산·문서 이력을 만들지 않는다. 일반 Enter는 draft를 XML에 확정한 후 sourceLeft / measured bottom+half-grid의 다음 내부 입력점으로 이동한다. 프로그램이 소비한 Enter는 프로그램 줄을 편집하고 Area 밖으로 나가지 않는다. 다음 Text는 다음 자식을 만든다. `=`은 명시 평가이고 일반 draft 입력은 이전 계산을 실행하지 않는다.

문서 이력은 실제 보이는 편집과 첫 자식 생성을 기록한다. source-only Enter/포커스 commit은 같은 내용을 한 번 더 되돌리는 빈 Undo 항목을 만들지 않는다. 각 영역 commit은 먼저 전체 후보를 검증하고 설치한다.

## 저장과 출력

새 자식은 owning source 안에서만 의미가 있는 `mathcraft-area-new-N` ID를 사용한다. MCDX export는 새 owner와 새 자식에 패키지 전체의 숫자 region-id/resultRef를 할당하고 Pending resultData를 만든다. 기존 자식 ID/ref와 알 수 없는 원본 sibling은 유지한다. 원본 자식 제거/이름 바꾸기, unknown 신규 영역, 보호/중첩 새 Area는 거절한다. 소유권과 좌표를 평면 Canvas에 복제하지 않는다.

실제 앱 입력으로 새 Area→`x:=3`→Enter→다음 `x=`→3을 확인하고, 자체 저장/계산 없는 재열기와 MCDX 생성/재계산을 검사한다. 새 빈 Area의 native 저장도 별도로 만든다. 자체 결과 JSON은 실행 입력으로 사용하지 않는다. 인쇄는 기존 Area vector snapshot을 사용하며 owning height와 retained collapse height를 구분한다.

## 검증과 한계

소스 고정/양 OS gate 및 XML/PDF 독립 검증은 `artifacts/rea/area-authoring-integration-20261019`에 기록한다. 이 문서의 최초 archive 상태는 검증 준비 중이며 최종 결과를 후속 기재한다.

**r140 최종 검증:** Windows workspace1166+backend2, peets workspace+backend1168, 실패0/ignored60. 두 OS fmt·workspace/all-target Clippy·backend Clippy·build, reference38saved/38input 모두 통과했다. Windows release도 빌드했다. 이전 Area/행렬에 새 빈 Area·새 자식 저작을 더한 native10건/양 OS, worksheet/header/footer/presentation/result50 XML part가 XSD 오류0이다. 원본1015/1016·실제 신규 Area/기존 Area/행렬·helper를 포함한16PDF는 PyMuPDF/pypdf 독립 추출·이미지0을 확인했다. 원본 앱/GUI/프린터를 실행한 검증과 구분한다.

Archive11,274,240bytes/SHA256 `8063abfd045c0144aa01920bb43b50b463510c36389d4672590014396653eff7`, manifestSHA `9183442a0ee271a9edcc8c7337d1d660f85b5ddd9af0ecebefbe7b0944838f9c`,381파일/178product Rust를 peets에서 재검증했다. 회수한716,800byte evidenceSHA `8c7366595a4f53b37fc96b712991d5191f4c5fc09d302f59a069fe1ff4a6a659`도 양끝이 같다. 모든 gate/산출물 hash는 최종 `validation.json`에 남겼다. 최종 문서 갱신과 이후 성능 작업은 이 archive 시점의 소스를 바꾸지 않는다.

Frozen Windows release는22,854,656bytes/SHA `5c1d13afee7013b0a501e6078efeee16b3310703b21761842e618fb42bd7dc5e`, CPU harness는23,542,272bytes/SHA `e55d87ab0070ce99bdc9ed4dab655e50c656af0999eeda88c4c6998cc93ae8dd`다. 직전 r137 release보다47,104bytes 증가했으며 전반적인 용량·지연 개선으로 포장하지 않는다.

원본 first character와 remaining string의 이력 분할 전체, native midpoint snap/tolerance, 모든 child collision/InsertSpace 확장, container 입력점의 Undo 위치 복원, 다중 child cut/paste·원본 보호는 남아 있다. collapse시 live ContentHeight와 cached actualHeight를 동일시하지 않으며 전체 WPF binding의 높이 전이는 미확정이다. 기본 렌더링·기본 입력·검사 수를40% 완성으로 환산하지 않는다.
