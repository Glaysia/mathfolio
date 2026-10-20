# 원본 정렬점과 화면·인쇄 통합

2026-10-16. [표·색상](0048-native-table-and-colors.md)과 [벡터 PDF](0049-vector-pdf-output.md)를 실제 1015 문서에 연결하면서 발견한 별도 좌표 오류를 교정했다. 창·원본 런타임·스크린샷을 사용하지 않고 REA의 정적 CIL, 제공 PDF의 문자 origin·도형 좌표, headless 입력·인쇄 검사를 사용했다.

## 저장 좌표의 의미

MCDX의 `top`을 모든 영역의 위쪽 끝으로 그리면 안 된다. 원본 `WhiteboardViewModel.ArrangeOverride`는 저장 위치에 개별 control의 `VerticalAlignmentOffset`을 더한다. EquationControl은 기준선의 음수, RichTextBase는 첫 글꼴 기준선과 control padding을 변환한 위치의 음수를 반환한다. SpecTableView는 두 번째 헤더 내용의 최대 바닥과 첫 데이터 행 내용의 최소 위쪽 사이 중점의 음수를 반환한다. 원본 토큰·IL SHA와 보존된 REA 증거는 [0048](0048-native-table-and-colors.md)의 anchor 후속 조사에 연결했다.

제공 원본 PDF의 첫 제목 baseline은 85.03881073 pt, 첫 `Pout`의 P baseline은 212.59814453 pt다. MCDX의 해당 top에 72/96 배율과 표준 위 여백을 적용한 값은 각각 85.03937/212.59843 pt다. 따라서 저장 좌표와 원본 문자 기준선이 일치하며 단순한 위쪽 위치로 처리한 이전 구현의 수직 오차를 설명한다.

수식의 독립 조판에서 baseline과 크기를 함께 측정한다. 화면은 기존 retained formula layout을 재사용하므로 anchor를 구하려고 매 프레임 새로 typeset하지 않는다. rich text와 표는 Arc로 보관한 동일 layout의 기준선을 사용한다. Spec Table의 각 행은 `max(baseline) + max(height-baseline)`의 높이로 계산하고 셀마다 기준선 차이만큼 이동한다. 표의 anchor도 테두리 행 상자가 아닌 실제 내용 배치 사각형 사이 중점으로 계산한다. 셀 숫자는 제공 PDF의 여러 자릿수 값에서 확인한 가운데 정렬을 적용한다.

원본 제목의 첫 글자는 저장 left에서 5 DIP, 원본 Pout의 P는 4 DIP 안쪽에 있다. 이 수평 여백은 제공 PDF의 origin 측정으로 확인했다. 직접 그 값을 지정한 원본 template literal을 발견했다는 주장은 하지 않는다. 수학 glyph의 얼굴은 독립 STIX이고, rich text는 설치된 시스템 글꼴을 사용한다. 원본 글꼴 프로그램을 복제하지 않는다.

## 적용과 저장

원본 출처가 있는 지원 영역에만 좌표 offset을 적용한다. 원본 archive·저장 top/left·계산 배치 순서와 새로 작성한 자체 영역의 기존 좌표 규칙을 바꾸지 않는다. 따라서 MCDX 재저장 때 보정량을 좌표에 다시 더하는 이중 변환이 없다. 활성 수식의 커서·선택도 같은 이동된 child UI에서 처리한다. 아래 화면의 원본 anchor가 화면 밖이더라도 해당 수식/표의 윗부분이 보일 수 있어 viewport 제외 검사도 높이만큼 보수적으로 잡는다.

인쇄 snapshot에는 측정한 높이와 정렬 offset을 함께 보관한다. 페이지 수·선택 영역 페이지·보이는 부분 제외 검사·실제 paint 모두 같은 보정 좌표를 사용한다. 인쇄는 수식 실행을 요청하지 않고 저장 결과를 그대로 사용한다. 원본 ZIP와 계산 원시 cache, 편집기 Undo 이력은 snapshot에 복제하지 않으며 native 서식 모델은 Arc로 공유한다.

## 오른쪽 여백 잘림

원본 1015 표의 오른쪽 끝은 약 800.172 pt이며 표준 본문 오른쪽 경계 약 771.024 pt를 넘지만 A3 종이 841.890 pt 안에 있다. 원본 PDF에는 마지막 awg 열이 정상 출력된다. 이전 인쇄 코드는 본문 경계에서 clip하여 마지막 열 전체를 없앴다. 가로 clip과 출력 대상 제외 기준을 실제 종이 끝으로 교정했다. 본문 여백은 배치 기준이며 종이 안의 내용을 자르는 가로 경계로 사용하지 않는다. 종이 밖 Draft 영역을 인쇄하는 기능을 새로 추가한 것은 아니다.

## 검증과 경계

회귀는 원본 수식 기준선의 화면·인쇄 일치, 자체 영역의 기존 배치 불변, 서로 다른 크기의 표 헤더/단위 셀의 행 기준선 일치, 실제 내용 사각형을 사용한 표 anchor, 오른쪽 여백과 종이 밖 영역의 구분, readonly 표·색상의 인쇄 연결을 검사한다. 실제 1015/1016의 cold open→print snapshot→벡터 PDF를 Windows와 peets에서 실행한 결과는 [CONFORMANCE](../CONFORMANCE.md)에 기록한다.

독립 글꼴의 전체 advance·행간·서식 및 native 표의 모든 cell sizing이 WPF와 동일하다는 의미는 아니다. 저장된 actualHeight를 보조 최소 크기로 쓰는 경로, 머리글/바닥글·Area·미지원 opaque 영역의 전체 정렬과 페이지 경계는 추가 검증 대상이다. Spec Table은 읽기 전용 복원이며 셀 편집과 열 정의의 계산 연결은 미구현이다.
