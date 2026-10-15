// arcana-desktop-rust CI: the three desktop platforms in parallel; the build fails if any fails.
//  * Linux  (bluesea, Docker): `bash scripts/ci-check.sh` in Dockerfile.ci --target ci, then a GUI
//    smoke test (real app under Xvfb + headless Weston, Dockerfile.ci --target gui), then Sonar.
//    Only this branch takes lock('ci-rust-build'): it shares bluesea's memory with the other
//    Rust repos (2026-09-24 overnight hang). The Windows / Mac agents are separate machines.
//  * Windows (label `windows`, office MSI laptop): scripts/ci-windows.ps1 — cargo test + release.
//  * macOS  (label `macos`, Mac mini): scripts/ci-native.sh — cargo test + release.
// Both native scripts install rustup for the build user if cargo is missing (idempotent).
pipeline {
    agent none
    options {
        timeout(time: 120, unit: 'MINUTES')
        buildDiscarder(logRotator(numToKeepStr: '20'))
    }
    stages {
        stage('Platforms') {
            parallel {
                stage('Linux') {
                    agent { label 'built-in' }
                    options { lock('ci-rust-build') }
                    environment {
                        IMG = "arcana-desktop-rust-ci:build-${env.BUILD_NUMBER}"
                        GUI_IMG = "arcana-desktop-rust-gui:build-${env.BUILD_NUMBER}"
                    }
                    stages {
                        stage('Check') {
                            steps {
                                sh '''
                                    set -e
                                    docker build -f Dockerfile.ci --target ci -t "$IMG" .
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
                        stage('GUI smoke (X11 + Wayland)') {
                            steps {
                                sh '''
                                    set -e
                                    docker build -f Dockerfile.ci --target gui -t "$GUI_IMG" .
                                    C="desktop-gui-$BUILD_NUMBER"
                                    docker rm -f "$C" >/dev/null 2>&1 || true
                                    rm -rf smoke-out
                                    set +e; docker run --name "$C" "$GUI_IMG"; RC=$?; set -e
                                    docker cp "$C":/app/smoke-out ./smoke-out 2>/dev/null || true
                                    docker rm -f "$C" >/dev/null
                                    exit $RC
                                '''
                            }
                            post { always { archiveArtifacts artifacts: 'smoke-out/*.png', allowEmptyArchive: true } }
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
                        stage('Architecture Qube') {
                            // Blocking, threshold 90 — same recipe as arcana-cloud-rust: copy the source IN via a
                            // tar stream (a -v bind of $WORKSPACE would resolve on the host and scan an empty
                            // tree) and the report OUT with docker cp. AQ_FRAMEWORK moves to rust-desktop once
                            // arch-qube ships that profile: the backend `rust` profile has no view/viewmodel
                            // layer, so its UI rules pass vacuously here (arcana-ui is never classified).
                            environment { AQ_FRAMEWORK = 'rust' }
                            steps {
                                sh '''
                                    AQ="arcana-arch-qube-desktop-${BUILD_NUMBER}"
                                    docker rm -f "$AQ" 2>/dev/null || true
                                    docker create --name "$AQ" --network devops_default -v /src -v /output \
                                        arcana.boo/arcana/arch-qube:latest \
                                        scan /src --framework "$AQ_FRAMEWORK" --no-ai --ci \
                                        --format json,markdown -o /output --threshold 90 || exit 1
                                    tar --exclude=./.git --exclude=./target --exclude=./.scannerwork \
                                        --exclude=./arch-qube-reports -C . -cf - . | docker cp - "$AQ":/src || exit 1
                                    docker start -a "$AQ"
                                    AQ_RC=$?
                                    mkdir -p arch-qube-reports
                                    docker cp "$AQ":/output/. arch-qube-reports/ 2>/dev/null || true
                                    docker rm -f "$AQ" 2>/dev/null || true
                                    cat arch-qube-reports/arch-qube.md 2>/dev/null | head -40 || true
                                    exit $AQ_RC
                                '''
                            }
                            post { always { archiveArtifacts artifacts: 'arch-qube-reports/*', allowEmptyArchive: true } }
                        }
                    }
                    post { always { sh 'docker rmi "$IMG" "$GUI_IMG" >/dev/null 2>&1 || true' } }
                }
                stage('Windows') {
                    agent { label 'windows' }
                    options { timeout(time: 90, unit: 'MINUTES') }
                    steps {
                        bat 'powershell -NoProfile -ExecutionPolicy Bypass -File scripts\\ci-windows.ps1'
                    }
                }
                stage('macOS') {
                    agent { label 'macos' }
                    options { timeout(time: 90, unit: 'MINUTES') }
                    steps {
                        sh 'bash scripts/ci-native.sh'
                    }
                    post { success { archiveArtifacts artifacts: 'dist/*.dmg', allowEmptyArchive: true } }
                }
            }
        }
    }
}
