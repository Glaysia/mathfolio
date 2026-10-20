# 행렬 결과 표시 범위와 전체 값 보존

2026-10-20. r218 고정 검증을 채택한 기록이다. 아래 기능 검증과 과거 고정 검증을 구분하며, 검사 수를 제품 완성률로 환산하지 않는다. 40% 목표는 미완료다. GUI·스크린샷·설치 Prime/CLR/WPF는 실행하지 않는다.

## 원본 계약과 구현 범위

[원본 행렬 표시 계약](../../artifacts/rea/matrix-result-display-next-20261020/README.md)과 [가져온 형식의 상속 경로](../../artifacts/rea/matrix-import-inheritance-next-20261020/REPORT.md)를 기준으로 결과 표시와 계산 값을 분리한다. 전체 행렬을 유지하면서 요청한 행·열 크기와 오프셋의 셀만 서식 처리한다. 원본 `size`/`offset` 순서는 행,열이며, 경계에서는 먼저 오프셋을 뒤로 옮겨 요청 크기를 유지한다. 요청 metadata 자체를 실제 크기로 정규화하지 않는다.

새 문서의 명시적인 생성 기본값은 12×12와 오프셋 0,0이다. 기존 native 문서는 실제 presentation과 로컬 resultFormat의 알려진 속성만 상속한다. 설정 부재를 12×12로 추정하지 않는다. source 행렬의 `ml:display`와 결과의 worksheet matrix format은 별개다.

결과의 오른쪽 아래 괄호를 드래그해 표시 크기를 조절한다. 생략 기호를 누르면 전체 범위와 현재 창을 나타내는 팝업이 열리고, 현재 창을 드래그해 숨겨진 행·열로 이동한다. 팝업 내부 가장자리 크기 조절과 ORIGIN이 입증된 인덱스 표시·숨기기, 키보드 navigator는 미완료다. 픽셀 크기·색상·hit tolerance·셀 단위 반올림은 독립 렌더링 정책이며 원본 픽셀 동등성을 주장하지 않는다. 전역 결과 펼치기 명령은 추가하지 않는다.

미리보기는 문서와 Undo를 바꾸지 않고, 확정은 표시 metadata만 한 번의 Undo로 기록한다. 계산 서명·전체 결과·Synchronized/Pending 상태·background generation을 바꾸지 않는다. Escape·포인터 손실·같은 크기의 새 결과 도착·source 수정·문서 교체·owner 소멸에는 미확정 상태를 취소한다. 확장된 결과에서 Enter를 누르면 실제 결과 높이 아래로 다음 입력 위치를 잡고, 영역 복제는 확인된 표시 범위를 새 owner로 옮긴다.

별도 `MatrixResultViews` DTO는 own 저장에 사용한다. native export는 source/owner/식/resultRef를 검증한 뒤 resultFormat의 요청 속성만 수정한다. 결과 XML과 계산 상태는 표시 변경만으로 다시 쓰지 않는다. Text의 copy-on-write와 결합되는 순서는 결과/옵션 → 행렬 format → Text다. Area 자식 override의 새 편집·저장은 지원하지 않으며 명시적으로 거절한다. 기존 Area 원문 보존과 새 결과 viewport 조작은 구분한다.

[원본 clipboard의 정적 계약](../../artifacts/rea/matrix-region-clipboard-next-20261020/README.md)에서 로컬 ResultMatrixFormat과 결과 값의 별도 직렬화를 확인했다. region Copy/Cut/Paste는 표시 요청만 새 Math owner에 옮기며 계산 결과 권한을 복사하지 않는다. 붙여넣기는 실제 결과가 생기기 전의 요청을 허용하고, DTO/type/map 예산은 문서 게시 전에 검사한다. 실제 projection 제한은 결과가 생겼을 때 적용한다. 원본 inherited-only XAML의 자동 materialization은 입증되지 않았으며, 확인된 native effective 속성을 복사하는 경로는 Mathcraft의 명시적 호환 정책이다.

인쇄는 화면과 동일한 불변 projection을 사용한다. 표시가 지원 범위를 벗어나면 잘못된 예전 셀을 출력하는 대신 오류를 반환한다. 임의 imported 형식·중첩 배열·원본 전체 lexical 규칙·원본 인쇄의 인덱스/포커스 동등성은 미완료다.

## 값·단위·비용

행렬 안의 첫 `unitedValue`를 scalar로 읽던 복원 오류를 수정했다. root의 실제 단위 wrapper만 분리하며 셀별 단위가 있는 행렬은 전체 형상을 유지한다. [같은 fixture의 수정 전/후 검사](../../artifacts/rea/matrix-native-units-r217-exact-fixture-red-green-20261020/receipt.json)에서 기존 두 collapse 오류와 수정 후 통과를 구분한다. r217 component 검증은 Windows 1981, peets 1982 정상 검사이며 전체 checkpoint 채택은 아니다.

새 flat matrix profile은 전체 65,536셀, XML 1MiB/262,144노드/깊이12의 별도 제한을 갖는다. 보이지 않는 셀도 처음 한 번 검증하며, 기존 v1 미리보기 wire의 제한을 완화하지 않는다. 표시 projection의 노드/텍스트 제한도 별개로 유지한다. 지원하지 않는 metadata를 만나 요청한 오프셋을 무시하고 예전 preview로 돌아가는 fallback은 금지한다.

50×50 native 결과에서 기존 전체 2,500셀 서식 처리와 새 기본 창 144셀을 계수했다. 100×100은 새 admission에서 전체 값을 보존하며, 반복 64회 조회에서 admission/projection 계수가 1/1로 유지된다. 전체 앱의 체감 지연·FPS·메모리·GPU 개선을 측정한 결과는 아니다.

## 검증 상태

현재 기록은 개발 중이다. [Enter·복제의 기존 제품 실패](../../artifacts/rea/matrix-continuation-r218-old-product-20261020.log)와 [동일 검사 수정 후 7건 통과](../../artifacts/rea/matrix-continuation-r218-repaired-product-20261020.log)를 보존했다. 새 export에 기본 format을 저장하지 않던 native cold 인쇄 회귀는 [통합 검사 실패](../../artifacts/rea/matrix-r218-integrated-preflight-20261020.log)에서 발견했고, [동일 기존 회귀의 수정 후 통과](../../artifacts/rea/matrix-fresh-defaults-r218-repaired-product-20261020.log)로 확인했다.

최종 양 OS 동일 소스 검사와 독립 native/own/PDF packet 결과는 아직 이 문서에 채택하지 않았다. 과거 r216의 192MCDX·10XML·120PDF 결과를 현재 소스의 새 검사로 재사용하지 않는다. 원본 runtime 재열기나 시각 동등성은 검증하지 않는다.

동결 전 로컬 사전 검사에서는 Windows workspace 2046건과 이후 clipboard 추가 경로를 확인했다. [새 placement packet의 독립 내용 검사](../../artifacts/rea/matrix-viewport-r218-placement-preflight-validated-20261020.json)는 MCDX4·own2·벡터 PDF2의 전체 수치 golden/0 images를, [XSD 검사](../../artifacts/rea/matrix-viewport-r218-placement-preflight-schema-20261020.json)는 XML24 parts를 통과했다. 첫 packet은 synthetic tall cache를 native baseline20에 놓아 상단 행이 페이지 밖으로 나간 실패로 보존한다. fixture baseline을350으로 옮긴 새 packet에서도 표시 창의 모든 값을 요구하는 같은 strict golden을 사용했으며, 제품 native 좌표를 clamp하거나 누락을 허용하지 않았다. 이 사전 packet은 아래 최종 동결 소스의 양 OS packet과 별개다.

duplicate classification의 최초 제안 검사2건은 cold saved map이 비어 있는 live fixture를 잘못 가정해 실패했다. 실제 document cache row를 설치한 동일 의미의 검사2건은 기존 제품에서 통과했으므로 제안된 classification 제품 패치는 채택하지 않았다. 최초 fixture 오류와 수정 후 양성 로그를 보존하며 제품 오류 수정 실적으로 세지 않는다.

## r218 채택 검증

동일한656파일 source archive로 Windows 2055, peets 2056 정상 검사(각 backend2 포함), ignored 101/99, 각48harness를 통과했다. fmt·workspace/backend Clippy·workspace build·reference conformance와 새 native4/own2/PDF2 packet을 OS별로 검증했다. 합계 MCDX8·own4·벡터 PDF4·XSD48parts이며 전체 보이는182/144셀을 두 PDF parser로 각각 대조했다. 원본/GUI·이미지 검토는 수행하지 않았다.

[채택 receipt](../../artifacts/rea/matrix-viewport-r218-integration-20261020/validation.json), [Windows 실행 파일](../../artifacts/rea/matrix-viewport-r218-integration-20261020/windows/mathcraft.exe)을 보존한다. 실행 파일은 빌드만 했으며 시작하지 않았다. source archive SHA는 `89742cadeeaeddae4ddf306fec5d7059ca0c4b4ea9492900ce04884c86813749`, source manifest SHA는 `435ef2465b90cc7335b6b1056c9a54492c619f81aa89d4aae45cccc9b2f3e5e7`다. 아래 문서 갱신은 검증 후 별도 receipt로 기록하며 archive 안의 당시 개발 문서는 이력으로 유지한다.

기능 상태는 EA-065만 부분 구현으로 올린다. EA-066 인덱스·EA-064 빈 셀 Ctrl+좌우 경계 이동은 여전히 미구현이다. 별도의 Ctrl+Shift+좌우가 일반 항 이동으로 잘못 처리되지 않게 한 guard는 원본 전체 scope 명령 구현으로 세지 않는다.555행(부분158/미구현397)과87가족(구현4/부분69/미구현14)은 서로 겹치며 완성률로 환산하지 않는다. 이전 전체192MCDX·10XML·120PDF packet은 이번에 재생성하지 않았다.
