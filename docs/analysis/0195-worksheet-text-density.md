# 확대 문서의 도표 글자 밀도

r207의 양 OS 고정 검증을 완료했다. GUI·스크린샷·설치 Mathcad 실행 없이 실제 Rust workspace 경로를 headless로 호출한다. [r206](0194-rich-text-source-integration.md)의 기능 범위와 전체 40% 목표는 별개다.

## 재현한 원인과 변경

[선명도 소스 조사](../../artifacts/rea/text-sharpness-next-20261020.md)는 문서 layer 확대가 모든 글자 생성 경로의 raster 밀도를 함께 올리지는 않는다는 점을 확인했다. 자체 수식·rich text는 이미 최종 밀도 helper를 쓰지만, 일부 도표 TextEdit·라벨·진단 문자열·인쇄 영역 밖 안내는 기본 창 배율에서 만든 glyph를 늘렸다. 수정 전 실제 workspace 테스트가 OS PPP1·문서125%의 x 라벨 UV 불일치로 실패했다.

- 자체 x/y 행 라벨, legacy 도표 표현식 두 입력란, trace 진단, 인쇄 영역 밖 안내를 기존 typography helper에 연결했다. 문서 배율이 1보다 클 때 Context PPP×문서 배율로 글자를 만들고 문서 DIP 좌표를 유지한다.
- 축 제목은 공개 `x_axis_label`/`y_axis_label`의 Galley 인자를 사용한다. 기존 String 경로의 폰트·색상·line-height·정렬 정책을 보존하며 축 배치·두께·Y축 회전은 기존 plot 코드가 담당한다. 의존성 소스나 전역 DPI를 변경하지 않았다.
- 입력란의 기존 widget ID·focus·char limit·이벤트·IME·확정/Undo 경로를 유지했다. 한 줄 입력의 trailing whitespace와 row-height 정책을 보존한다. x/y 행 라벨은 RichText의 공개 변환을 사용해 색상 override·valign·폰트 정책을 맞춘다.

## 개발 검사

[수정 전 재현](../../artifacts/rea/text-density-r207-before-repair-20261020.log)은 실제 workspace→plot→라벨을 그린 결과와 해당 정확한 LayoutJob을 최종 밀도에 직접 layout한 glyph UV를 비교한다. helper만 따로 호출해 통과시키는 검사가 아니다. 출력은 tessellate하고 FullOutput을 해제한 뒤 assertion한다.

[축 제목까지 연결한 검사](../../artifacts/rea/text-density-r207-titles-preflight-20261020.log)는 관련 7개 검사를 통과했다. 새 actual workspace 검사는 OS PPP1/1.5/2×문서 확대1/1.25/2의 9조합에서 8개 실제 텍스트를 확인한다. 원문 JSON·dirty·Undo/Redo·계산 제출 generation·실행 영역과 계산 준비 full-DOM counter가 변하지 않는다. Y축 제목 -π/2 회전도 확인한다. 이는 모든 종류 XML 호출의 instrumentation 또는 GPU 측정이 아니다.

별도 검사는 기본/override 색상·세 valign·좁은 폭에서 x/y 행 라벨의 stock 서식/크기 정책을, override font·alpha 색·line spacing·한글/이모지/여러 줄 조건에서 제목의 전체 stock LayoutJob을 대조한다. legacy 입력의 끝 공백과 한 줄 job도 검사한다.

개발 중 실패를 보존했다. 첫 후보 테스트는 private counter 접근으로 컴파일되지 않아 기존 counter 소유 모듈 아래에 테스트를 등록했다. 첫 OS 배율 확장 검사에서는 oracle의 두 번째 RawInput에 native PPP를 전달하지 않아 테스트가 1로 돌아갔다. 두 프레임에 동일 native PPP를 전달한 뒤 검사했다. stock 라벨 비교 테스트의 첫 `run_ui` closure는 tuple을 반환해 컴파일되지 않았으며 외부에 결과를 저장하도록 고쳤다. 이 이력은 최종 고정 검증과 구분한다.

## 남은 범위

plot 의존성의 눈금·범례·hover 문자열, 전체 오류 라벨은 이 변경의 수정 범위가 아니다. [축 제목 후속 조사](../../artifacts/rea/plot-axis-title-density-next-20261020/README.md)에 공개 주입 API가 있는 제목과 없는 경로를 구별했다. 글자 모양의 원본 픽셀 동일성·실제 GPU 지연·100% 배율에서 느끼는 선명도 문제를 해결했다고 판정하지 않는다. bitmap glyph atlas 자체를 벡터 글자 그리기로 바꾼 변경도 아니다. PDF 벡터 출력은 별도의 기존 경로다.

[문단 정적 계약](../../artifacts/rea/rich-text-paragraph-next-20261020.md)은 Enter/Shift+Enter·문단 삭제·Run 순번의 새 원본 근거다. 조사 완료가 문단 편집 구현 완료를 뜻하지 않는다. 기능별 기존 부분/미구현 판정과40% 목표를 유지한다.

## 양 OS 고정 검증

[고정 검증 결과](../../artifacts/rea/text-density-integration-20261020-r207/validation.json)는 `checkpoint_accepted: true`다. frozen source와 현재 제품을 대조했다. 최종 gate/probe/validator 재실행은 없으며 위 개발 실패는 별도 기록이다.

| 항목 | 결과 |
|---|---|
| source archive | 13,864,960 bytes · SHA `36cc4b931d2c1b513cde9d1e1ea3596bca3cd0ff3c91d56338badc1d03e2acbf` |
| source manifest | 550 files / product Rust288 · SHA `24f5b8a080e23bb0db30835871f3ba8921b6d35a95102d961cbf2ec1eb461f38` |
| Windows / peets | 각각 1705 PASS / 실패0 / ignored89 |
| format / Clippy / build / reference | 양 OS PASS; Windows release PASS |
| MCDX / standalone XML | 142 / 10 · XSD862parts PASS |
| 벡터 PDF | 70 files · image0 · fitz+pypdf text PASS |
| 실행 파일 | [r207](../../artifacts/rea/mathcraft-text-density-r207-20261020.exe) · 25,368,576 bytes · SHA `83f3cedc46e5cbeb36b8af7621c95c0a7fc6243636e97465032d08fa9b8e5366` |
| validation | SHA `41ed842635df03fc52018143e2369d4452d6fdbb63b002cf134ddc565c85cc26` |

기능 목록은 HD076의 지원 깊이를 갱신했다. 세부555행은 부분156/미구현399, 상위87행은 구현4/부분69/미구현14를 유지한다. 이 분류를 전체 완성률로 환산하지 않는다.
