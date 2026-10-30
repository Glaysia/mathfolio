# 0215 — Mathfolio 0.1.1 이름 변경 검증

2026-10-30. 사용자 요청에 따라 저장소와 제품을 Mathfolio로 변경하고 0.1.1로 올린다.
기준은 기존 `v0.1.0` / `f7afd06`이며, 해당 태그와 이미 배포한 파일은 수정하지 않는다.

## 변경과 호환

GitHub 저장소는 같은 저장소 ID를 유지한 `Glaysia/mathfolio`다. 네이티브 앱 제목,
파일 대화상자, 인쇄·문서 메타데이터, CLI, 6개 Rust 크레이트와 디렉터리,
SDK의 패키지·클래스·생성 파일, 현재 사용 문서를 새 이름으로 정리했다.
새 자체 문서의 기본 확장자는 `.mathfolio`다. 자세한 계약은
[RENAME-0.1.1.md](../RENAME-0.1.1.md)를 따른다.

기존 `.mathcraft` 문서와 저장 확장자, 문서 버전 2와 SDK 프로토콜 1을 유지했다.
캐시 해시 도메인·Area 식별자·구조화 클립보드 접두사를 변경하지 않았다.
SDK는 이전 `MathcraftEditor`와 `exportDocument("mathcraft")`도 별칭으로 받는다.
새 MCDX의 application은 Mathfolio, appVersion은 현재 Cargo 버전에서 가져온다.
원본 문서의 기존 패키지 메타데이터를 임의로 바꾸지는 않는다.

## 검증

동일한 제품 소스 520개(Rust·자산·vendor·Cargo)를 고정하고 Windows와 peets에서
검증했다. 문서·테스트 호스트 후속 수정과 제품 소스를 구분해 해시를 대조했다.

- Windows: fmt, clippy(진단 feature 포함), workspace·backend 테스트, dev/release 빌드,
  CLI 버전과 reference conformance 통과. 테스트 2,197개 통과, 105개 ignored.
- Kubuntu: 같은 검사·빌드·CLI/reference 검사 통과. 2,198개 통과, 103개 ignored.
- 두 OS의 일반 release로 실제 1015 MCDX와 기존 확장자의 합성 문서를 열고
  `--smoke-test` 종료 코드 0 확인. Linux는 소유한 Xvfb 세션으로 실행했다.
- 이름 변경 전 작성된 합성 행렬 문서 바이트를 새 참조 fixture로 고정했다.
  두 확장자의 대소문자 변형을 포함한 저장·재열기에서 캐시 7행과 원래 서명을 보존했다.
- 최적화 배포 WASM을 Chrome 156의 소유한 임시 headless 프로필에서 로드했다.
  기존 SDK 통합 검사와 새 제품 제목·별칭·기존 캐시 호환을 포함해 65개 통과,
  page error 없음, 브라우저 소유 프로세스 종료 확인.

REA 브라우저 근거:
`ev_388e1b953bd400a88d8f52f16002e10bfccb3ec2cda4bab431e5648859161ef7`.
검사 초안은 `inspectResults`의 새 계산 결과와 저장 캐시를 혼동했다. 저장 결과는
문서의 `calculation_cache`로 확인하도록 검사만 수정했고, 실패 근거도 보존했다.
저장 결과를 새 계산 바인딩으로 취급하지 않는다.

계산 엔진·편집기·호환 계층의 Rust 소스 116개는 이름을 정규화한 뒤 이전 태그와
바이트 단위로 같음을 확인했다. 릴리즈 프로필과 네이티브 렌더러 선택은 바꾸지
않았다. 이번 이름 변경에서 새 스크롤 지연 측정을 수행하지는 않았다.
물리 IME·실제 플러그인 호스트 검증은 여전히 별도 범위다.

## 용량

MB는 1,000,000바이트 기준이다. 새 이름·0.1.1로 전체를 재빌드했다.

| 산출물 | 바이트 |
| --- | ---: |
| Windows GUI exe | 26,698,752 |
| Windows CLI exe | 4,999,680 |
| Linux GUI | 23,544,088 |
| 최적화 WASM | 13,255,770 |
| SDK 전체 디렉터리 | 15,492,342 |
| SDK tar.gz | 6,152,066 |

WASM SHA-256:
`2733ca57c2cffcb6a434917bde680345f4563bd18b23666da2fe152079324f43`.
SDK tar.gz SHA-256:
`6006a734501a9cdaf9ca17d6a9758316e4f5b83d1f25487dbc2924d5e4d23ac8`.

검증 로그·고정 소스 목록·실제 브라우저 근거·SDK 파일별 해시와 배포 체크섬은
ignored `artifacts/rename-0.1.1`, `dist/wasm-sdk-size.json`, `dist/release-0.1.1`에 둔다.
