# 벡터 PDF 출력

2026-10-16. GUI, 스크린샷, 원본 프로그램 실행 없이 구현하고 검사했다. 원본의 페이지 범위·용지·snapshot 계약은 [0033](0033-print-and-page-contract.md)을 따른다. PDF 처리에는 `pdf` skill을 읽었고, 최초 생성 전 artifact marker를 한 번 실행했다. 사용자의 GUI·스크린샷 금지 지시에 따라 화면이나 렌더 이미지를 보지 않고 PDF 객체·문자·도형 좌표와 headless 회귀를 검사했다.

## 교정한 문제와 증거

기존 출력은 egui 도형을 소프트웨어로 페이지 크기의 RGB 이미지로 만든 뒤 PNG의 압축 데이터를 PDF의 `/Image` 객체로 넣었다. 그러면 PDF 확대에 따른 글자 품질, 검색·선택·복사가 모두 제한된다. 이것은 계산 문제와 별개인 출력 백엔드 문제다.

사용자가 제공한 PDF의 기존 조사 `artifacts/rea/pdf-comparison-20261016/metadata.json`은 다음 차이를 기록한다. 이 파일을 생성하기 위해 이번 작업에서 원본을 다시 실행하지 않았다.

| PDF | SHA-256 | 조사 결과 |
| --- | --- | --- |
| 원본 `1015_LCC_S_mathcad.pdf` | `e0e340254feed28722df827393a0ba2524beedc45847b3dcb2d43002b499ec68` | 2쪽, Type0 글꼴과 텍스트·벡터 도형, 전체 페이지 이미지 없음 |
| 기존 Mathcraft `1015_LCC_S_mathcraft.pdf` | `52321f630a62df694a9e2abd673eb0f35168955fd0f59ddbf3146bd7f3156329` | 2쪽 각각 3508×4961 RGB 이미지 1개, 추출 문자·벡터 도형 없음 |

원본 PDF의 구조가 특정 PDF 라이브러리나 글꼴 파일을 복제해야 한다는 뜻은 아니다. 독립 구현은 기존 Mathcraft 조판을 PDF의 텍스트·경로로 기록한다.

## 구현

`printing::render_vector_pages`는 egui의 두 번의 headless layout pass에서 두 번째 `ClippedShape` 목록과 `FontDefinitions`를 보관한다. egui_plot의 이전 layout 의존성을 정착시키는 기존 warm-up은 유지한다. 이 경로는 96 DIP/in 배율에서 조판하며, 삼각형 소프트웨어 래스터화·페이지 RGB 버퍼·PNG 인코딩을 실행하지 않는다. 기존 `render_pages`는 Windows GDI 인쇄용 PNG fallback을 보존한다. `write_pdf`는 두 경로 모두 보관된 벡터 정보를 쓴다. Linux CUPS의 PDF도 같은 백엔드를 사용한다.

- 페이지는 DIP×72/96으로 `/MediaBox`를 만들고, 좌상단 원점·아래 방향 Y 좌표를 PDF 변환 행렬로 대응시킨다. 문서의 물리 크기를 보존하며 fit-to-page 배율을 추가하지 않는다.
- 사각형·둥근 모서리·회전, 선·열린/닫힌 경로, 원·타원, 이차/삼차 베지어를 PDF 경로로 기록한다. 도형별 clip, 선 너비, RGB, 투명도를 보존한다.
- 텍스트는 egui의 실제 행·글리프 baseline을 사용한다. 같은 locked HarfRust 0.12로 shaped GID·클러스터를 복원하므로 `ffi` 같은 합자도 올바른 윤곽선과 `ffi` 유니코드 문자열로 연결한다. 수학 문자 중 BMP 밖의 코드 포인트는 UTF-16 surrogate pair로 매핑한다.
- 독립 STIX 및 설치된 시스템 글꼴을 사용한다. Mathcad 글꼴이나 제품 리소스를 복사하지 않는다. egui의 글꼴 fallback 순서, font tweak 크기·offset, 색상과 회전, synthetic italics를 출력에 반영한다.
- 일반 TrueType는 Type0/CIDFontType2 + `FontFile2` + `CIDToGIDMap` + `ToUnicode`로 포함한다. 사용 GID와 복합 글리프 의존성만 `glyf`에 남기며, 기존 GID·metrics·hint 프로그램은 보존한다. `loca`를 다시 만들고 SFNT table checksum 및 `head.checkSumAdjustment`를 갱신한다. PDF에 이미 shaped GID와 위치가 있으므로 GSUB/GPOS/MATH 등 화면용 조판 표는 넣지 않는다. no-subsetting 플래그가 있는 개별 TrueType는 원본 글꼴 전체를 포함한다.
- TTC/CFF는 skrifa로 사용 글리프의 벡터 윤곽선을 해석해 Type3 글꼴에 포함한다. 255개씩 별도 font resource를 만들고 각 코드에 `ToUnicode`를 붙인다. Linux의 NotoSansCJK TTC도 검색·선택 가능한 텍스트를 유지한다. 글꼴 전체를 이미지나 윤곽선 한 장으로 바꾸는 방식이 아니다.
- 글자 배경·밑줄·취소선은 벡터 도형으로 보존한다. egui 장식 삼각형의 단색 alpha fringe는 벡터 Gouraud soft mask로 기록해 페이지 bitmap을 만들지 않는다.
- 출력 전에 모든 내용을 인코딩하고, 같은 폴더 staging 파일에 저장한 뒤 교체한다. 지원하지 않는 내용이 있으면 오류를 반환하고 기존 PDF를 보존한다.

PDF export 때문에 수식을 계산하지 않는다. worksheet snapshot·저장 결과·계산 상태 처리는 기존 print caller의 책임이며 이 백엔드는 표시 목록만 소비한다.

## 검사와 측정

다음 검사는 창·GPU·프린터를 시작하지 않는다.

```powershell
cargo test -p mathcraft-app printing:: --locked --offline -- --nocapture
cargo test -p mathcraft-app printing::vector_pdf::tests::write_vector --locked --offline -- --ignored --nocapture
cargo clippy -p mathcraft-app --all-targets --locked --offline -- -D warnings
```

일반 printing 회귀 **12개 통과**, artifact 생성용 **2개 ignored**이며 두 ignored 검사도 별도로 실행해 통과했다. 경로·곡선·clip·alpha·선 너비·텍스트 회전·Unicode·합자·배율과 기존 Windows raster fallback을 검사한다. 복합 글리프가 있는 `AéÅﬃ`의 원본/부분 글꼴 윤곽선 및 advance를 skrifa로 비교하고 전체 font checksum 상수를 검증한다. GPU callback·texture mesh·비유한 좌표는 명시 오류 및 기존 출력 보존을 검사한다.

ignored fixture는 `artifacts/rea/vector-pdf-backend.pdf`, `vector-cff-korean.pdf`다. 후자는 peets의 독립 시스템 글꼴 `/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc`를 읽기 전용 복사해 검사했다. 이 글꼴과 PDF는 Git에 넣지 않는다. PyMuPDF 1.27.2.2, pypdf 6.15.0, FontTools로 확인한 결과를 `artifacts/rea/vector-pdf-backend-validation.json`에 기록했다.

| fixture | 크기 | 독립 검사 |
| --- | --- | --- |
| 수식·합자·회전·장식 | 95,144 bytes | 1쪽, 300×450 pt, 이미지 0개, 벡터 도형 21개, 3개 Type0/embedded subset font. `office affine ffi`, `α = 3.5 Ω`, `𝑥 = √2`, 회전 및 장식 텍스트를 두 라이브러리에서 정확히 추출·검색 |
| Noto TTC/CFF 한글 | 6,189 bytes | 1쪽, 이미지 0개, Type3 벡터 font. 두 라이브러리에서 `한글 표 인쇄 ABC 123` 정확 추출, 한글 검색 성공 |

첫 fixture에서 전체 TrueType를 포함했을 때 **1,868,911 bytes**였으나 부분 글꼴로 **95,144 bytes**가 되었다(약 94.9% 감소). 부분 글꼴들의 실제 nonempty glyph 수는 9·10·9개였고, 원래 GID 범위는 각각 1261·6760·5344로 보존했다. FontTools가 포함된 font program의 checksum과 구조를 읽을 수 있음을 확인했다. 독립 검사에서는 선 2.5 DIP→1.875 pt, alpha 128/255, 첫 텍스트의 x=15 pt·크기=18 pt와 clip 연산도 확인했다.

이는 백엔드 fixture의 결과다. 실제 모든 원본 문서의 조판 동일성이나 드라이버의 출력 성공을 뜻하지 않는다. 문서별 print snapshot 통합 검사는 별도 근거로 판단한다.

후속 실제 파일 검사에서는 1015/1016 MCDX 두 개를 Windows와 peets에서 각각 A3 2쪽으로 출력했다. 4개 모두 전체 페이지 이미지 없이 1,117개 벡터 도형과 검색/선택 텍스트를 가지며 표의 55개 값·한글·색상을 확인했다. 최종 1015 PDF는 Windows 378,847 bytes, peets 196,284 bytes다. 같은 파일의 초기 전체 TrueType 포함 단계는 19,741,153 bytes였으며 최종 subset 경로에서 크기를 줄였다. Windows 1015의 벡터 생성·파일 기록은 한 번의 CPU 측정에서 약 68 ms였고, cold open과 계획 수립·사용자 GUI 지연은 이 시간에 포함하지 않는다. 최종 소스의 두 OS 검사와 PDF SHA는 [CONFORMANCE](../CONFORMANCE.md)를 따른다.

## 현재 한계

GPU callback, 외부 texture/brush, blur, 다색 mesh gradient, UV 색상 callback과 안쪽/바깥쪽 일반 Path stroke는 명시 오류다. 지원하지 않는 내용을 성공한 PDF에서 조용히 생략하지 않는다. 오른쪽→왼쪽 텍스트, 일부 다중 글리프 클러스터, 사용자 지정 가변 글꼴 축, elided 문자열은 아직 지원하지 않아 오류를 반환한다. 현재 검사는 정상 LTR 한글·라틴·수학 기호와 합자에 한정한다. fallback 글꼴에도 문자가 없으면 오류다. TTC/CFF Type3는 원래 글꼴 프로그램의 힌팅을 포함하지 않으며 벡터 윤곽선과 Unicode를 보존한다.

스캔 이미지·인쇄 드라이버·실물 출력 품질, 모든 Rich Text/표/도표의 페이지 경계와 글꼴별 시각적 동일성, 원본의 proprietary math face와 독립 STIX의 모양 차이는 이번 백엔드 검증으로 완료를 주장하지 않는다. GUI와 스크린샷은 사용하지 않았다.
