<svelte:head>
    <title>개인정보처리방침 - 참나무 지도</title>
</svelte:head>

<div class="privacy-wrap">
    <div class="privacy-inner">
        <h1>개인정보처리방침</h1>
        <p class="last-updated">최종 수정일: 2026년 4월 13일</p>

        <p>
            참나무 지도(이하 "서비스")는 이용자의 개인정보를 중요하게 생각하며,
            「개인정보 보호법」 및 「위치정보의 보호 및 이용 등에 관한 법률」에 따라
            다음과 같이 개인정보처리방침을 운영합니다.
        </p>

        <section>
            <h2>1. 수집하는 개인정보 항목</h2>
            <h3>가. 소셜 로그인 시 수집 항목</h3>
            <table>
                <thead>
                    <tr>
                        <th>제공자</th>
                        <th>수집 항목</th>
                    </tr>
                </thead>
                <tbody>
                    <tr>
                        <td>Google</td>
                        <td>닉네임(이름)</td>
                    </tr>
                    <tr>
                        <td>네이버</td>
                        <td>닉네임</td>
                    </tr>
                    <tr>
                        <td>카카오</td>
                        <td>닉네임</td>
                    </tr>
                </tbody>
            </table>

            <h3>나. 서비스 이용 시 수집 항목</h3>
            <ul>
                <li>저장 위치 정보: 위치명, 위도·경도 좌표, 메모, 저장 일시</li>
                <li>실시간 위치 정보(GPS): 지도 현위치 표시 및 나침반 기능에 한해 일시적으로 사용되며, 서버에 저장되지 않습니다.</li>
            </ul>

            <h3>다. 자동 생성 정보</h3>
            <ul>
                <li>로그인 인증 토큰(JWT): 이용자 기기의 로컬 스토리지에 저장되며 서버에는 보관되지 않습니다.</li>
                <li>서비스 접속 로그 (서버 오류 발생 시 한정)</li>
            </ul>
        </section>

        <section>
            <h2>2. 개인정보 수집 목적</h2>
            <ul>
                <li>회원 식별 및 로그인 서비스 제공</li>
                <li>저장 위치 데이터의 등록·조회·삭제 기능 제공</li>
                <li>현재 위치 기반 지도 서비스 및 나침반 기능 제공</li>
            </ul>
        </section>

        <section>
            <h2>3. 개인정보 보유 및 이용 기간</h2>
            <table>
                <thead>
                    <tr>
                        <th>항목</th>
                        <th>보유 기간</th>
                    </tr>
                </thead>
                <tbody>
                    <tr>
                        <td>회원 정보 (이메일, 닉네임 등)</td>
                        <td>회원 탈퇴 시까지</td>
                    </tr>
                    <tr>
                        <td>저장 위치 정보</td>
                        <td>이용자가 삭제하거나 회원 탈퇴 시까지</td>
                    </tr>
                    <tr>
                        <td>실시간 GPS 위치</td>
                        <td>서버에 저장하지 않음 (기기 내 일시적 사용)</td>
                    </tr>
                </tbody>
            </table>
            <p>법령에 따라 보존이 필요한 경우 해당 기간 동안 보관할 수 있습니다.</p>
        </section>

        <section>
            <h2>4. 위치정보 수집 및 이용</h2>
            <p>
                서비스는 이용자의 현재 위치를 지도에 표시하고 나침반 방향 추적 기능을 제공하기 위해
                기기의 GPS 정보를 수집합니다.
            </p>
            <ul>
                <li>위치 정보는 이용자가 직접 저장하는 경우에만 서버에 기록됩니다.</li>
                <li>저장하지 않은 실시간 위치는 서버에 전송되거나 저장되지 않습니다.</li>
                <li>위치 정보 수집은 이용자가 언제든지 기기 설정 또는 브라우저 권한에서 거부할 수 있습니다.</li>
            </ul>
        </section>

        <section>
            <h2>5. 제3자 제공</h2>
            <p>
                서비스는 이용자의 개인정보를 원칙적으로 외부에 제공하지 않습니다.
                단, 지도 서비스 제공을 위해 카카오맵 API를 사용하며,
                이 과정에서 카카오의 개인정보처리방침이 적용될 수 있습니다.
            </p>
            <ul>
                <li>카카오맵 API: 지도 표시 목적, 현재 위치는 카카오 서버로 전송되지 않음</li>
            </ul>
        </section>

        <section>
            <h2>6. 이용자의 권리</h2>
            <p>이용자는 언제든지 다음 권리를 행사할 수 있습니다.</p>
            <ul>
                <li>개인정보 열람 요청</li>
                <li>개인정보 수정 요청</li>
                <li>개인정보 삭제 요청 (회원 탈퇴)</li>
                <li>개인정보 처리 정지 요청</li>
            </ul>
            <p>저장된 위치 정보는 서비스 내에서 직접 삭제할 수 있습니다.</p>
        </section>

        <section>
            <h2>7. 쿠키 사용</h2>
            <p>
                서비스는 검색 패널 상태 복원을 위해 쿠키(<code>search_panel_hidden</code>)를 사용합니다.
                이 쿠키에는 개인정보가 포함되지 않으며, 브라우저 설정에서 거부할 수 있습니다.
            </p>
        </section>

        <section>
            <h2>8. 개인정보 보호책임자</h2>
            <table>
                <tbody>
                    <tr>
                        <td>서비스명</td>
                        <td>참나무 지도</td>
                    </tr>
                    <tr>
                        <td>문의</td>
                        <td>newrps@gmail.com</td>
                    </tr>
                </tbody>
            </table>
            <p>
                개인정보 관련 문의, 열람·정정·삭제 요청은 위 연락처로 하시면
                지체 없이 처리하겠습니다.
            </p>
        </section>

        <section>
            <h2>9. 방침 변경</h2>
            <p>
                본 방침은 법령·서비스 변경에 따라 수정될 수 있으며,
                변경 시 서비스 내 공지를 통해 안내합니다.
            </p>
        </section>

        <div class="back-btn-wrap">
            <a href="/" class="back-btn">← 지도로 돌아가기</a>
        </div>
    </div>
</div>

<style>
    .privacy-wrap {
        min-height: 100vh;
        background: #f8f9fa;
        padding: 40px 16px;
        box-sizing: border-box;
    }
    .privacy-inner {
        max-width: 720px;
        margin: 0 auto;
        background: white;
        border-radius: 12px;
        padding: 40px 32px;
        box-shadow: 0 2px 12px rgba(0,0,0,0.08);
    }
    h1 {
        font-size: 24px;
        font-weight: 700;
        margin-bottom: 6px;
        color: #1a1a1a;
    }
    .last-updated {
        font-size: 13px;
        color: #888;
        margin-bottom: 24px;
    }
    section {
        margin-top: 32px;
    }
    h2 {
        font-size: 17px;
        font-weight: 700;
        color: #2d8a4e;
        margin-bottom: 12px;
        padding-bottom: 6px;
        border-bottom: 2px solid #e8f5e9;
    }
    h3 {
        font-size: 14px;
        font-weight: 600;
        color: #444;
        margin: 14px 0 8px;
    }
    p, li {
        font-size: 14px;
        line-height: 1.8;
        color: #333;
    }
    ul {
        padding-left: 20px;
        margin: 8px 0;
    }
    li { margin-bottom: 4px; }
    table {
        width: 100%;
        border-collapse: collapse;
        font-size: 14px;
        margin: 10px 0;
    }
    th, td {
        border: 1px solid #e0e0e0;
        padding: 10px 12px;
        text-align: left;
    }
    th {
        background: #f5f5f5;
        font-weight: 600;
        color: #555;
    }
    code {
        background: #f0f0f0;
        padding: 2px 6px;
        border-radius: 4px;
        font-size: 13px;
    }
    .back-btn-wrap {
        margin-top: 40px;
        text-align: center;
    }
    .back-btn {
        display: inline-block;
        padding: 10px 24px;
        background: #2d8a4e;
        color: white;
        border-radius: 8px;
        text-decoration: none;
        font-size: 14px;
        font-weight: 600;
    }
    @media (max-width: 600px) {
        .privacy-inner {
            padding: 24px 16px;
        }
    }
</style>
