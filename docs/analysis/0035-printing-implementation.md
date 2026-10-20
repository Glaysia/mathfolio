# 워크시트 인쇄 구현

2026-10-16. 원본 계약은 [0033](0033-print-and-page-contract.md), 성능 변경은 [0034](0034-rendering-and-calculation-cost.md)에 기록한다. GUI·스크린샷·원본 런타임·실물 인쇄를 사용하지 않고 구현했다.

초기 구현의 `printing.rs`는 GPU 없이 기존 수식·문자·도표 도형을 조판하고 소프트웨어로 rasterize하여 PNG/PDF에 넣었다. 이후 [0049](0049-vector-pdf-output.md)에서 PDF/CUPS는 경로·검색 텍스트·부분 글꼴의 벡터 출력으로 교체했다. Windows 물리 인쇄의 PNG fallback만 150/300/600 dpi를 선택하며 페이지당 4천만 픽셀로 제한한다. PDF에는 bitmap 해상도 선택을 노출하지 않는다. 페이지별 글꼴/도표 축 warm-up, 출력 합계 256 MiB·최대 1,000페이지 제한은 유지한다.

파일 메뉴 및 Ctrl+P는 인쇄 설정을 연다. 전체/현재/연속 범위/선택 영역이 걸친 페이지를 구분하며 원본과 같이 `1,3` 쉼표 목록은 받지 않는다. 선택된 수식만 잘라 출력하지 않는다. 페이지 번호와 전체 수는 원본 문서 전체 기준을 유지한다. 인쇄 snapshot은 계산을 실행하지 않으며 수동 상태의 이전 결과도 그대로 사용한다. 종이 밖 Draft 내용은 페이지 수 산정과 출력에서 제외한다. 종이 안 오른쪽 여백에 걸친 표는 [0052](0052-native-alignment-and-print-integration.md)에 따라 종이 끝에서만 자른다. UI의 선택 테두리·캐럿·도표 편집 필드·resize handle은 출력하지 않는다.

용지·방향·사용자 여백은 문서에 저장하며 PageGeometry의 격자에 맞춘 본문 높이를 출력에서도 사용한다. 기존 MCDX의 A1–A5/Letter/Legal/Ledger와 Standard/Narrow/Wide/Legacy/Custom을 읽는다. 여백 값은 좌·상·우·하 DIP이며 Custom Thickness의 1/2/4 숫자를 읽는다. 자체 문서에서 설정한 용지·방향·여백은 MCDX의 `pageModel`에 내보낸다. 정보용 `page-width`/`page-height`에는 points를 사용한다.

실제 출력 명령은 사용자 인쇄 버튼에서만 시작한다. Windows는 시스템 기본/선택 프린터와 PrintDocument를 사용하고 Linux는 CUPS의 `lp`/`lpr`로 PDF를 보낸다. Windows의 전송용 PowerShell은 독립 앱의 System.Drawing interop이며 Mathcad DLL을 로드하지 않는다. 문서명·프린터명은 실행 코드에 이어 붙이지 않고 별도 JSON/프로세스 인자로 전달한다. 문서 크기와 방향을 넘기며 임의 fit-to-width를 추가하지 않는다. 프린터 목록 조회·페이지 조판·전송은 worker thread에서 처리한다. 프린터 목록은 10초 timeout으로 제한한다. 실제 프린터 드라이버/대기열/장치 출력은 아직 검증하지 않았다.

테스트는 PDF의 페이지 구조·MediaBox·이미지 해상도와 압축 형식, 숫자/단위/글꼴/도형의 픽셀 값, 페이지 범위·선택 페이지·실제 물리 크기, CUPS 옵션, source/result 보존을 확인한다. MCDX 용지/여백 export의 구조 검사와 전체 Windows/Linux 검증 결과는 [CONFORMANCE](../CONFORMANCE.md)를 따른다.

저장 날짜는 이후 [0038](0038-saved-date-fields.md)에 따라 실제 저장 시각의 7개 형식으로 화면/출력에 연결했다. 전체 문화권·달력 범위는 부분 구현이다. 전체 native 영역·접힌 Area·명시 페이지 나눔·첫 페이지 별도 머리글·모든 프레임/머리글 눈금 설정과 실제 드라이버별 동작은 미완료다. 기존 워크시트가 읽기 전용 요약으로 표시하는 미지원 native 영역은 인쇄에서도 전체 원본 표현으로 복구되지 않는다. 이 경계를 포함해 인쇄 기능은 통합 목록에서 부분 구현으로 유지한다.
