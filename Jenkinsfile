// arcana-desktop-rust CI. Same gate as `bash scripts/ci-check.sh`, run inside Dockerfile.ci.
// Shares lock('ci-rust-build') with the other Rust repos so concurrent Rust builds cannot
// exhaust bluesea's memory (2026-09-24 overnight hang).
pipeline {
    agent none
    options {
        lock('ci-rust-build')
        timeout(time: 60, unit: 'MINUTES')
        buildDiscarder(logRotator(numToKeepStr: '20'))
    }
    stages {
        stage('Rust CI') {
            agent any
            environment { IMG = "arcana-desktop-rust-ci:build-${env.BUILD_NUMBER}" }
            stages {
                stage('Check') {
                    steps {
                        sh '''
                            set -e
                            docker build -f Dockerfile.ci -t "$IMG" .
                            C="desktop-ci-$BUILD_NUMBER"
                            docker rm -f "$C" >/dev/null 2>&1 || true
                            set +e; docker run --name "$C" "$IMG"; RC=$?; set -e
                            docker cp "$C":/app/clippy-report.json . 2>/dev/null || true
                            docker cp "$C":/app/lcov.info . 2>/dev/null || true
                            docker rm -f "$C" >/dev/null
                            exit $RC
                        '''
                    }
                }
                stage('SonarQube Analysis') {
                    steps {
                        withSonarQubeEnv('SonarQube') {
                            sh '''
                                set -e
                                sonar-scanner \
                                  -Dsonar.projectKey=desktop-rust-app \
                                  -Dsonar.projectName="Desktop Rust App" \
                                  -Dsonar.projectVersion="$(git rev-parse --short HEAD)" \
                                  -Dsonar.sources=crates,plugins,xtask \
                                  -Dsonar.exclusions=**/target/** \
                                  -Dsonar.scm.disabled=true \
                                  -Dsonar.rust.clippy.enabled=false \
                                  -Dsonar.rust.clippyReport.reportPaths=clippy-report.json \
                                  -Dsonar.rust.lcov.reportPaths=lcov.info
                                TOKEN="${SONAR_AUTH_TOKEN:-$SONAR_TOKEN}"
                                RT=.scannerwork/report-task.txt
                                [ -f "$RT" ] || { echo "report-task.txt not found"; exit 1; }
                                CE=$(grep '^ceTaskId=' "$RT" | cut -d= -f2-)
                                AID=""
                                for i in $(seq 1 60); do
                                    R=$(curl -s -u "$TOKEN:" "$SONAR_HOST_URL/api/ce/task?id=$CE")
                                    ST=$(echo "$R" | grep -o '"status":"[A-Z_]*"' | head -1 | cut -d'"' -f4)
                                    if [ "$ST" = "SUCCESS" ]; then AID=$(echo "$R" | grep -o '"analysisId":"[^"]*"' | head -1 | cut -d'"' -f4); break; fi
                                    if [ "$ST" = "FAILED" ] || [ "$ST" = "CANCELED" ]; then echo "CE task $ST"; exit 1; fi
                                    sleep 5
                                done
                                [ -n "$AID" ] || { echo "CE task did not finish"; exit 1; }
                                G=$(curl -s -u "$TOKEN:" "$SONAR_HOST_URL/api/qualitygates/project_status?analysisId=$AID")
                                GST=$(echo "$G" | grep -o '"status":"[A-Z]*"' | head -1 | cut -d'"' -f4)
                                echo "Quality gate: ${GST:-UNKNOWN}"
                                [ "$GST" = "OK" ] || { echo "$G"; exit 1; }
                            '''
                        }
                    }
                }
            }
            post { always { sh 'docker rmi "$IMG" >/dev/null 2>&1 || true' } }
        }
    }
}
