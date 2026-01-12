## Tuack-NG
[![FOSSA Status](https://app.fossa.com/api/projects/git%2Bgithub.com%2Ftuack-ng%2Ftuack-ng.svg?type=shield)](https://app.fossa.com/projects/git%2Bgithub.com%2Ftuack-ng%2Ftuack-ng?ref=badge_shield)


tuack-ng 项目是重构后的 tuack 项目，旨在提供更加高效和轻量的出题体验。

详见：[项目 / 计划：tuack-ng](https://pulsar33550336.github.io/2025/12/10/%E9%A1%B9%E7%9B%AE-%E8%AE%A1%E5%88%92%EF%BC%9Atuack-ng/)

备忘：PKGBUILD 的 `package` 应该改成：

```bash
package() {
    install -Dm755 tuack-ng -t "$pkgdir/usr/bin"
    install -Dm644 LICENSE "$pkgdir/usr/share/licenses/$pkgname/LICENSE"
    
    install -dm755 "$pkgdir/usr/share/tuack-ng/templates/"
    cp -r templates/* "$pkgdir/usr/share/tuack-ng/templates/" 2>/dev/null || true
    
    find "$pkgdir/usr/share/tuack-ng/templates" -type d -exec chmod 755 {} \;
    find "$pkgdir/usr/share/tuack-ng/templates" -type f -exec chmod 644 {} \;
}
```


[![FOSSA Status](https://app.fossa.com/api/projects/git%2Bgithub.com%2Ftuack-ng%2Ftuack-ng.svg?type=large)](https://app.fossa.com/projects/git%2Bgithub.com%2Ftuack-ng%2Ftuack-ng?ref=badge_large)