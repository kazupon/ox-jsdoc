import { execFileSync } from 'node:child_process'

const command = value => {
  execFileSync(value, {
    stdio: 'inherit',
    shell: true
  })
}

if (process.platform === 'win32') {
  command('vp run vize:lint')
} else {
  command('vp run vize:lint')
  command('vp exec vize check --tsconfig tsconfig.json src')
}
