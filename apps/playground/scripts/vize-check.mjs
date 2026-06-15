/**
 * vize lint/check for playground.
 *
 * @author kazuya kawaguchi (a.k.a. kazupon)
 * @license MIT
 */

import { execFileSync } from 'node:child_process'

const command = value => {
  execFileSync(value, {
    stdio: 'inherit',
    shell: true
  })
}

command('vp run vize:lint')
command('vp exec vize check --tsconfig tsconfig.json src')
