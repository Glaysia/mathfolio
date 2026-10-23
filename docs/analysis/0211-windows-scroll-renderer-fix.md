# Windows 스크롤 수정: CPU 어댑터의 2D 표시 경로

2026-10-23. 같은 Windows의 원본 Mathcad는 부드럽다는 사용자 확인을 기준으로 수정했다. Mathcraft의 DX12 CPU 경로를 CPU 2D rasterizer와 native pixel surface로 대체했고, 사용자가 수정 후보를 직접 사용하고 “이제 버벅이지 않는데?”라고 확인했다.

## 변경

Windows의 어댑터가 Cpu이고 창이 opaque 2D일 때 Mathcraft가 명시적으로 opt in한다. vendored library의 기본값은 false다. hardware GPU는 기존 WGPU, Kubuntu는 기존 Glow 경로를 유지한다.

기존 eframe/winit 창·이벤트 루프·입력 hook·IME·휠 큐·viewport 명령·계산 작업자는 유지한다. 같은 egui mesh·UV·clip·색상·texture delta·physical pixel scale을 direct CPU SIMD rasterizer에 전달하고 softbuffer Windows pixel surface에 표시한다. 이 창에는 DXGI swapchain을 configure/present하지 않는다. PDF는 기존 독립 vector exporter를 사용한다.

egui-wgpu0.36.2와 egui_software_backend `c70e02d3d95c75a82733deb97e516b13a6c86dc6`를 기반으로 한다. 후자는 egui0.36 upload 배치에 포팅했다. primitive hash cache를 끄고 partial upload 뒤 UV-zero texel·sampler 갱신 및 큰 clipped paper 처리를 보완했다. 원본 Mathcad 코드·DLL은 제품에 포함하지 않는다. 출처·라이선스는 vendor의 MATHCRAFT.md와 NOTICE.md에 기록했다.

## 동일 실행체의 실제 창 비교

진단 실행체 SHA256 `ebbd508efb8c154234ed9b8dbb2cd6398424e7205dea7fa5104a2ceaf95e1d48` 하나에서 fallback on/off를 바꿨다.1180×900·native PPP1·동일1015 문서 SHA256 `217994a631de58e096a3cfdb8a8251aca91501e62151f869950b99f738fc18b5`다. 빌드와 Linux 검사 종료 후 순차 실행했다.

각 실행112 native 입력 프레임 중17–96번80개 스크롤 프레임의 시작 간격이다. 같은 아래/위 합성 휠을 실제 App hook과 ScrollArea로 전달했다. 문서·저장 결과·계산 제출 세대·Undo/Redo·dirty 불변을 통과했다.

| 조건 | 중앙값 | p95 | 최대 |
| --- | ---: | ---: | ---: |
| 기존 WGPU 강제,1015 100% | 218.968ms | 249.990ms | 250.125ms |
| CPU 2D,1015 100% | 6.464ms | 7.768ms | 10.199ms |
| CPU 2D,같은 조건 반복 | 5.872ms | 7.190ms | 8.187ms |
| CPU 2D,1015 200% | 4.116ms | 5.885ms | 6.851ms |
| CPU 2D,1016 100% | 6.020ms | 7.974ms | 9.370ms |

첫 최종100% 표본의 raster 중앙값4.305ms·native present 호출 중앙값0.418ms였다. 동일 실행체에서 표시 경로 변경의 효과를 확인했다. 하드웨어 부재 자체 때문에 지연이 불가피하다는 이전 설명과 구분한다.

이는 native 창/surface의 raw-input 시작 간격이며 모니터 scanout FPS·OS 마우스 수신 지연 또는 모든 문서의 상한 보장은 아니다. 원본 Mathcad의 부드러움은 사용자 관찰이다. 원본은 실행했지만 자동화의 화면 캡처·좌표 확보가 실패해 원본의 자동화 FPS 측정은 얻지 못했다. [0210 정적 REA](0210-mathcad-scroll-comparison.md)는 visible-pages 차이 갱신 근거로 보존한다.

## 표시·회귀 확인

같은 실행체의 GPU/CPU in-app capture를 비교했다.1,062,000px의 RGB 평균 절대 차이는 채널별0.0424/0.0200/0.0382(255단계),8단계를 넘는 차이는233px였다. 완전히 동일한 픽셀 출력이라고 주장하지 않는다. 수식·표·색상과200% 확대 이미지를 직접 확인했다. 앱의 그리기 결과이며 Windows Graphics Capture 성공을 뜻하지 않는다.

회귀3검사는 연속 partial upload의 새 픽셀,100,000px paper의 PPP2 clipping, 반투명 도형의 premultiplied 색상을 확인한다. Windows2183·peets2181 정상 검사, fmt·workspace/backend/painter Clippy·빌드·참조38건이 통과했다. 양 OS native-profile feature도 검사했다. 일반 release는 native-profile 없이 만든다.

원자료는 로컬 `artifacts/rea/software-renderer-20261023/`에 보존한다. 양 OS frozen source archive SHA256은 `73ecf839b80c7f5620e60ac5f220c79fe7e92dc9d7b928ec9d001daf923d87ef`다. 이후 문서·출처 기록만 추가했으며 runtime source는 동일하다.

현재 Mathcraft는 managed texture와2D mesh를 사용한다. 새 경로는 custom GPU callback·외부 GPU user texture·투명 창·depth·MSAA용이 아니다. opt-in 계약에 범위를 명시했다. 완료 범위는 보고된 Windows 스크롤 지연 수정이며 UI 디자인 개편은 후속 계획이다.
