# 환경 조사

확인일: 2026-10-15. 비밀키와 인증 토큰은 기록하지 않는다.

| 항목 | 확인 결과 |
| --- | --- |
| 개발 PC | 현재 Windows PC |
| 개발 경로 | `C:\Users\harry\Projects\mathcraft` |
| Git | 2.52.0.windows.1 |
| Windows Cargo | 1.99.0, CLI/GUI 빌드 및 CLI 실행 확인 |
| Windows MSVC | Build Tools 17.14.37710.0, MSVC 14.44.35207, SDK 10.0.26100.0, `C:\BuildTools\Mathcraft2022` |
| Linux 대상 | SSH 별칭 `peets`, hostname `5950xlinux` |
| Linux OS | `/etc/os-release`: Ubuntu 26.04.1 LTS |
| KDE | `/usr/bin/plasmashell` 존재. GUI 세션 실행 검증 전 |
| Linux Rust | Rust/Cargo 1.99.0 설치 완료, CLI/GUI 빌드 및 CLI 실행 확인 |
| Linux REA/Node/npm | 비대화형 SSH PATH에서 찾지 못함 |
| Mathcad 분석 대상 | 이 PC의 `C:\Program Files\PTC\Mathcad Prime 12.0.0.1`, 실제 모듈 정적 검사 시작 |
| 로컬 REA | 6.5.0 설치 및 Codex MCP 등록 완료, 현재 세션은 CLI 사용 |
| 로컬 네이티브 분석 도구 | Ghidra 12.1.4 / JDK 21.0.12.1+1 사전 검사 통과. Windows 혼합 DLL 제한 있음 |
| GitHub | gh 활성 계정 Glaysia, 저장된 토큰 무효. 원격 생성/push 미수행 |

## 설치 파일

- 파일: `pim_installmgr_mathcad.exe`
- 크기: 36,876,832 bytes
- ProductName: Mathcad Prime 12.0.0.1
- ProductVersion: 12.0.0.1
- FileDescription: PTC installation assistant
- CompanyName: PTC Inc.
- SHA256: `1690E359F68503DC4958BEABB33CFC70E888F78ECA2DE07E78CCC5F5FAA4B2D9`

위 버전은 설치 도우미의 메타데이터다. 설치된 Mathcad의 버전이나 실제 엔진 포함 여부를 보장하지 않는다. 설치 프로그램 실행 및 설치는 수행하지 않았다.
