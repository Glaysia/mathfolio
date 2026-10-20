# 첫 입력·확정 계산 입력·포커스 통합 검증

2026-10-19. 원본 계약은 [0042 일반 수식 확정](0042-edit-commit-contract.md), [0036 프로그램 Enter·행렬 Tab](0036-program-lines-and-matrix-tab.md), [0057 단축키·literal 아래첨자](0057-shortcut-and-subscript-contract.md)를 따른다. 이번 r93은 새 원본 기능 추가가 아니라 첫 입력에서 불필요한 전체 준비와 첫 포커스의 키 누락을 고친 작업이다. GUI·스크린샷·원본 프로세스·일반 앱 main·실제 프린터는 사용하지 않았다.

## 수정과 보존한 의미

[0089](0089-input-intent-and-focus.md)는 키 해제·커서 이동·선택·Copy 요청과 실제 편집을 구분한다. 보기와 설정 draft를 여는 리본도 계산 입력을 만들지 않는다. Tab·Ctrl+Shift+C는 행렬 확장·Column 삽입이 가능하므로 수정 전 상태를 보존한다. Ctrl+-는 이름의 literal suffix를 편집하고 zoom을 바꾸지 않는다.

첫 포커스 frame의 focus-lock 설정을 egui가 받아들이지 않은 뒤 기본 Tab traversal이 수식보다 먼저 키를 가져가는 오류를 실제 worksheet 경로에서 재현했다. 수식이 소유한 navigation을 처리하도록 교정했으며 첫 Tab의 새 행 확장과 첫 Shift+Tab의 이전 셀 이동을 검사한다. popup·pointer press에서는 해당 포커스 회수를 적용하지 않는다. 모든 OS 포커스·접근성 조합의 동등성은 미검증이다.

[0091](0091-lazy-committed-baseline.md)은 첫 draft 전에 이전 전체 XML을 만드는 대신 계산 source의 raw immutable snapshot을 보관한다. 일반 Enter는 사용하지 않은 이전 snapshot을 버리고 새 입력만 준비한다. draft를 유지한 F5 등이 이전 입력을 요구할 때에만 snapshot을 준비한다. Auto OFF·Manual·disabled·Undo·archive 변경·잘못된 XML과 수식 오류의 기존 공개 시점을 유지한다. snapshot은 native archive의 모든 바이트를 소유하며 저장 문서를 대신하지 않는다.

## 실제 열기 뒤 입력 측정

[0090](0090-first-open-input-cost.md)의 save→fresh open→빈 frame→focus→첫 이벤트→Enter 경로를 사용했다. 기존 모든 editor를 미리 만든 stress 측정과 구분한다. 최종 harness와 현재 소스에서 빌드한 Windows harness의 SHA가 일치한다. 같은 PC에서 before/after 순서를 교대해 각각 5회, 각 실행 36case를 검사했다.

| 문서 | 첫 Text frame 전→후 | Enter frame 전→후 | 각 실행 Text+Enter 합의 median 전→후 |
| --- | ---: | ---: | ---: |
| 4,000 scalar, 저장 cache 있음 | 66.081→2.954 ms | 15.196→69.195 ms | 81.277→71.854 ms |
| 4,000 scalar, 저장 cache 없음 | 70.319→3.362 ms | 14.932→74.848 ms | 85.190→78.210 ms |
| 원본 1015 | 6.017→0.592 ms | 1.447→4.737 ms | 7.464→5.349 ms |
| 원본 1016 | 5.488→0.637 ms | 1.318→4.645 ms | 6.764→5.227 ms |

캐시 있는 4,000영역 Arrow release는 64.528→0.942 ms였다. after 180case의 첫 이벤트 준비 호출은 모두 0회였다. 비편집 Arrow press/release·IME preedit는 Enter까지 준비 0회, Text/Paste/IME commit은 Enter에서만 1회다. draft 중 generation·실행 불변, 저장 표시 유지·Enter 단일 제출을 검사한다. peets에서도 같은 소스로 별도 36case가 같은 counter 계약을 통과했다. Windows와 Linux 시간을 서로 성능 비교하지 않는다.

비용 일부가 Enter로 옮겨졌으며 큰 문서 확정 지연은 여전히 남는다. 행렬과 open 단계·worker 잔여 시간·phase 분해·전체 표본은 0090을 따른다. 이 CPU test profile 결과를 실제 데스크톱/GPU frame rate·전체 계산 완료·20% 완성률로 확대하지 않는다.

## 동일 소스와 양 OS 회귀

Windows/peets 각각 **workspace 738개 + backend 2개 = 740개 통과·실패 0개·28개 ignored**다. app 334개, compat 249개, document 70개, editor 72개, core 13개다. fmt·workspace all-target Clippy와 별도 backend library/test Clippy(`-D warnings`)·workspace 빌드를 통과했다. Windows는 release로 빌드했으며 frozen 원본 참조 38개는 saved/input 양쪽 38/38·차이 0이다.

source archive는 ignored `artifacts/rea/mathcraft-input-baseline-20261019-r93.tar`, SHA-256 `1e4d903b380f86eb784e97a8c93b5b30927f644a6b75d8bc3e59a4d088946c93`, 7,464,960 bytes다. 제품 Rust 103개·OSS backend Rust 5개를 포함한 manifest 126파일을 archive·Windows·peets 실행 후 소스와 바이트 대조했다. peets fresh root는 `/home/peets/Projects/mathcraft-input-baseline-20261019-r93`다.

Windows release `target/release/mathcraft.exe`는 **22,008,832 bytes**, SHA-256 `3c9b5d684ef5b3f8aeee6284e0ba184e4f80c9fe165db703b34922abe97626e4`다. 일반 앱은 실행하지 않았다.

양 OS 각각 native 9건·PDF 7건을 검사했다. 1015/1016 실제 표의 80 계산 행·42 A 수정·64개 저장 표시, 지역 함수·행/열 대입·구조 연산자·단위와 자체 재열기 계산 0회의 회귀를 유지했다. MCDX 18개·XML 72파트의 설치 XSD 오류는 0개이며 실제 문서별 XamlPackage 27개는 원본과 같다. PDF 14개는 이미지 0개·두 reader의 검색 텍스트·원본 표 숫자/색상/기준점 검사를 통과했다. 원본 전체 배치·모든 글꼴·물리 프린터 동등성은 미검증이다.

## 기록과 남은 일

검증 root는 ignored `artifacts/rea/input-baseline-integration-20261019`다. `source-validation.json`은 archive·소스·실행 파일·각 검사 요약을, `schema-validation.json`과 `pdf-verified/validation.json`은 native/PDF 결과를 담는다. `peets/verification.json`의 산출물 37개·표준 로그 5개·별도 actual-open 로그 1개는 수집 후 SHA를 다시 확인했다.

Windows 로그는 `input-baseline-{windows-fmt,windows-clippy,windows-tests,backend-windows,backend-windows-clippy,windows-build,windows-reference,windows-probes}-20261019.log`, peets 표준 로그는 `input-baseline-peets-{check,backend-clippy,reference,probes,pdfs}-20261019.log`다. `open-input-peets-after-20261019.log`가 별도 lifecycle 원시 로그이며 Windows 5회 교대 표본은 `open-input-alternating-20261019.{json,log}`다. before·pre-navigation 후보·최종 after 실행 파일을 따로 보존했다.

다음 병목은 cold Enter의 전체 worksheet 문맥 라벨 처리와 표시 트리 복원이다. 기존 generation·취소·Manual/F5·오류 순서를 보존하는 경로를 먼저 조사한다. 기능 단일 기준 [FEATURES](../FEATURES.md)의 553행은 부분 구현 135·미구현 418로 유지한다. 사용자가 평가한 11%에서 목표 20%를 향한 진행이며 전체 20% 달성을 판정하지 않는다.
