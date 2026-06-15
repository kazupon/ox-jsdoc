import { execFileSync } from 'node:child_process'

const command = value => {
  execFileSync(value, {
    stdio: 'inherit',
    shell: true
  })
}

// vize check は Windows (vp check/vpr check経路) で //?/D:/... を含む
// tsconfig 参照の解決不具合により TS5058 を再現するため、一時回避として
// Windows では lint のみ実行する。
if (process.platform === 'win32') {
  command('vp run vize:lint')
} else {
  command('vp run vize:lint')
  command('vp exec vize check --tsconfig tsconfig.json src')
}
