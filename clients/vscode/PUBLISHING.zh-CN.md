# 发布 LSPF Analysis

[English](PUBLISHING.md) · [简体中文](PUBLISHING.zh-CN.md)

扩展以 `meymchen.lspf-analysis` 的身份发布到 Visual Studio Marketplace
和 Open VSX。第一个版本是 `0.1.0`，走预发布渠道。

发布由 [`release-vscode.yml`](../../.github/workflows/release-vscode.yml)
完成。本文依次说明一次性配置、发布前验证凭据、发布版本、故障排查，
以及在流水线失败时如何在本地构建和手动发布。

## 一次性配置

### 仓库前提

仓库必须公开。`linux-arm64` 构建使用 GitHub 托管的 ARM runner，
流水线依赖的 environment 保护规则和构建证明（attestation）
在私有仓库上只对付费计划开放。

### GitHub environment

在 **Settings → Environments** 中创建名为 `vscode-release` 的 environment：

- 在 **Deployment branches and tags** 中只允许匹配 `vscode-v*` 的 tag。
- 建议把自己设为 **Required reviewers**。这样推送 tag 后流水线会在发布前暂停，
  可以先下载构建产物检查，再批准发布。
- 添加变量 `AZURE_CLIENT_ID` 和 `AZURE_TENANT_ID`，取值见下文 Entra 应用。
- 添加 secret `OVSX_PAT`，取值见下文 Open VSX。

另外可以用 tag ruleset 限制谁能创建 `vscode-v*` tag，作为第二道防线。

### Visual Studio Marketplace

发布使用 Microsoft Entra ID 和 GitHub 的 OIDC token，任何地方都不保存
Marketplace token。认证链路如下：

1. GitHub 为 job 签发 OIDC token；
2. Entra 应用的 federated credential 接受这个 token，`azure/login` 登录成功；
3. Azure DevOps 为这个应用分配 profile ID；
4. 这个 profile ID 是 `meymchen` publisher 的 Contributor。

应用不需要加入任何 Azure DevOps 组织，也不需要 Azure 订阅中的角色。

Azure DevOps 的全局 PAT 于 2026 年 12 月 1 日停用，
新配置不要退回到 `VSCE_PAT`。

当前使用的应用是 `lspf-analysis-marketplace-publisher`。下面的步骤用于
在新租户中重建，或在应用丢失后恢复。命令需要 Azure CLI，并先用
`az login --tenant <tenant-id>` 登录。

#### 1. 创建专用的 Entra 应用

使用专门为发布创建的应用，不要复用 Azure DevOps service connection
自动创建的应用：删除 service connection 时，那个应用会被一起删除。

```console
az ad app create --display-name lspf-analysis-marketplace-publisher \
  --sign-in-audience AzureADMyOrg --query appId -o tsv
az ad sp create --id <app-id>
```

应用不需要 secret、证书或 Redirect URI。把两个值填入 environment 变量：

| 值 | 来源 | environment 变量 |
| --- | --- | --- |
| 租户 ID | `az account show --query tenantId -o tsv` | `AZURE_TENANT_ID` |
| 应用 ID | 上面 `az ad app create` 的输出 | `AZURE_CLIENT_ID` |

```console
gh variable set AZURE_TENANT_ID --env vscode-release --body <tenant-id>
gh variable set AZURE_CLIENT_ID --env vscode-release --body <app-id>
```

不要把 Object ID、subscription ID 或 secret ID 当作租户 ID 填入。
填错时 `azure/login` 会报 `AADSTS90002: Tenant ... not found`。可以用下面的
命令检查租户 ID，返回 `200` 才正确：

```console
curl -s -o /dev/null -w "%{http_code}\n" \
  https://login.microsoftonline.com/<tenant-id>/v2.0/.well-known/openid-configuration
```

#### 2. 添加 federated credential

先确认仓库 OIDC token 的 subject 格式：

```console
gh api repos/meymchen/lspf-analysis/actions/oidc/customization/sub
```

本仓库返回 `"use_immutable_subject": true`，subject 中的用户名和仓库名
会带上数字 ID。Entra 门户中的 “GitHub Actions deploying Azure resources”
模板只会生成旧格式，因此不能使用这个模板。

把下面的内容保存为 `credential.json`：

```json
{
  "name": "github-lspf-analysis-vscode-release",
  "issuer": "https://token.actions.githubusercontent.com",
  "subject": "repo:meymchen@86772442/lspf-analysis@1358382712:environment:vscode-release",
  "audiences": ["api://AzureADTokenExchange"]
}
```

然后添加：

```console
az ad app federated-credential create --id <app-id> --parameters credential.json
```

subject 的前缀就是上面命令返回的 `sub_claim_prefix`，后面接
`:environment:vscode-release`。`86772442` 是用户 `meymchen` 的 ID，
`1358382712` 是仓库的 ID。subject 不匹配时，`azure/login` 会报
`AADSTS700211: No matching federated identity record found`。

#### 3. 在 publisher 上授权

1. 取得应用的 profile ID。它由 Azure DevOps 分配，不是 Entra 中的任何 ID。
   应用只能通过 GitHub Actions 登录，所以要在 workflow 中查询：按下文
   [验证发布凭据](#验证发布凭据)运行检查，`marketplace` job 会打印
   `id`，这就是 profile ID。查询使用的命令是：

   ```console
   az rest -u https://app.vssps.visualstudio.com/_apis/profile/profiles/me \
     --resource 499b84ac-1321-427f-aa17-267ca6975798
   ```

2. 打开 [publisher 管理页][publishers]，在 `meymchen` 的 **Members**
   中用这个 ID 添加成员，角色选择 **Contributor**。
3. 重跑检查中失败的 job。`vsce verify-pat` 输出
   `The Personal Access Token verification succeeded for the publisher
   'meymchen'` 即完成。

官方说明见[自动发布到 Visual Studio Marketplace][auth]。

### Open VSX

1. 使用 GitHub 账号登录 [open-vsx.org][open-vsx]，并在个人资料中签署
   Eclipse Foundation Open VSX Publisher Agreement。
2. 在设置中创建访问 token，然后创建 namespace：

   ```console
   npx ovsx create-namespace meymchen -p <token>
   ```

3. 把 token 保存为 environment secret `OVSX_PAT`。

发布第一个版本后，改用 trusted publishing。Open VSX 只接受已有发布版本的
扩展注册。在 **Settings → Trusted Publishers** 中注册 owner `meymchen`、
仓库 `lspf-analysis`、workflow `release-vscode.yml` 和 environment
`vscode-release`，然后删除 secret `OVSX_PAT` 并吊销 token。没有这个 secret
时，`ovsx` 会改用 job 的 OIDC token 发布。切换后先按下一节验证一次。

## 验证发布凭据

`vscode-release` 只允许 `vscode-v*` tag 部署，所以凭据只能在 tag 触发的
workflow 中验证。下面的方法只检查发布权限，不上传任何内容。

1. 从 `main` 新建一个临时分支，删除 `release-vscode.yml`，避免临时 tag
   触发正式发布。
2. 添加 `.github/workflows/check-release-credentials.yml`，其中 action 的
   版本与 `release-vscode.yml` 保持一致：

   ```yaml
   name: Check release credentials

   on:
     push:
       tags: ["vscode-vcheck-*"]

   permissions: {}

   jobs:
     marketplace:
       runs-on: ubuntu-latest
       timeout-minutes: 10
       environment: vscode-release
       permissions:
         contents: read
         id-token: write
       defaults:
         run:
           working-directory: clients/vscode
       steps:
         - uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v7.0.1
           with:
             persist-credentials: false
         - uses: actions/setup-node@820762786026740c76f36085b0efc47a31fe5020 # v7.0.0
           with:
             node-version: 22
             package-manager-cache: false
         - run: npm ci --ignore-scripts
         - uses: azure/login@a641126d1b8aa4d1fa005f4f92df94a3a4c4c906 # v3.1.0
           with:
             client-id: ${{ vars.AZURE_CLIENT_ID }}
             tenant-id: ${{ vars.AZURE_TENANT_ID }}
             allow-no-subscriptions: true
         - run: >-
             az rest -u https://app.vssps.visualstudio.com/_apis/profile/profiles/me
             --resource 499b84ac-1321-427f-aa17-267ca6975798
             --query '{id: id, displayName: displayName}'
         - run: npx vsce verify-pat --azure-credential meymchen

     open-vsx:
       runs-on: ubuntu-latest
       timeout-minutes: 10
       environment: vscode-release
       permissions:
         contents: read
       defaults:
         run:
           working-directory: clients/vscode
       steps:
         - uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v7.0.1
           with:
             persist-credentials: false
         - uses: actions/setup-node@820762786026740c76f36085b0efc47a31fe5020 # v7.0.0
           with:
             node-version: 22
             package-manager-cache: false
         - run: npm ci --ignore-scripts
         - run: npx ovsx verify-pat meymchen
           env:
             OVSX_PAT: ${{ secrets.OVSX_PAT }}
   ```

3. 提交后只推送 tag，不推送分支：

   ```console
   git tag vscode-vcheck-1
   git push origin vscode-vcheck-1
   ```

4. 查看运行结果。`open-vsx` 输出 `PAT valid to publish at meymchen`，
   `marketplace` 的最后一步成功，说明两边都能发布。修改 Azure 或 GitHub
   的配置后，用 `gh run rerun <run-id> --failed` 重跑即可，变量和 secret
   在运行时读取，不需要新 tag。
5. 验证结束后删除临时 tag 和分支：

   ```console
   git push origin --delete vscode-vcheck-1
   git tag -d vscode-vcheck-1
   ```

只推 tag 时，远端不会有这个分支，它只存在于本地。

## 发布版本

1. 同时修改 `package.json` 和 `package-lock.json` 中的版本号，例如
   `npm version 0.1.1 --no-git-tag-version`。
2. 完成 changelog 条目。标题写成 `## 0.1.1` 或 `## 0.1.1 (preview)`，
   并去掉 `unreleased` 标记。changelog 会打包进 VSIX，之后的修改不会
   进入这个版本。
3. 把改动合并到 `main`。
4. 给合并后的提交打 tag 并推送：

   ```console
   git tag vscode-v0.1.1
   git push origin vscode-v0.1.1
   ```

流水线随后会：

- 检查 tag 与版本号一致、tag 指向的提交在 `main` 上，以及 changelog
  中有这个版本的完整条目；
- 运行单元测试，并为 `win32-x64`、`win32-arm64`、`darwin-x64`、
  `darwin-arm64`、`linux-x64`、`linux-arm64`、`alpine-x64` 和
  `alpine-arm64` 各构建一个 VSIX；
- 在能原生构建的目标上启动打包后的服务器，确认它能运行；
- 如果设置了 required reviewer，等待批准；
- 把所有包发布到 Marketplace 和 Open VSX；
- 创建 GitHub release，附上所有包、以 changelog 条目作为说明，
  并生成构建证明。

次版本号为奇数（如 `0.1.x`）时发布到预发布渠道，为偶数（如 `0.2.x`）时
发布到正式渠道。两个仓库都会跳过已发布的版本，所以部分上传后重跑失败的
job 是安全的。

修改 workflow 或打包脚本的 pull request 只运行构建，不会发布。想在其他时候
试构建，可以手动运行：

```console
gh workflow run release-vscode.yml --ref main
```

手动运行和 pull request 都不检查 tag 与版本号是否一致，也不检查 tag
是否在 `main` 上，这两项只在 tag 触发时执行。

### 发布后检查

在两个仓库上确认扩展 ID、版本号、渠道、目标平台和 changelog。
需要修正的包只能用新版本号重新发布。

## 故障排查

| 出错的步骤 | 错误信息 | 原因和处理 |
| --- | --- | --- |
| `azure/login` | `AADSTS90002: Tenant ... not found` | `AZURE_TENANT_ID` 不是租户 ID，改为应用 Overview 中的 Directory (tenant) ID |
| `azure/login` | `AADSTS700211: No matching federated identity record found` | federated credential 的 subject 不匹配，按[第 2 步](#2-添加-federated-credential)使用带数字 ID 的格式 |
| 查询 profile | `VSS011031: There is no profile for the authenticated user` | 应用在 Azure DevOps 中没有 profile。本仓库使用 service connection 自动创建的应用时遇到过，改用[第 1 步](#1-创建专用的-entra-应用)的专用应用后解决 |
| `vsce verify-pat` 或 `vsce publish` | `needs the following permission(s) on the resource /meymchen` | profile ID 不是 publisher 的 Contributor，见[第 3 步](#3-在-publisher-上授权) |
| `vsce verify-pat` 或 `vsce publish` | `InvalidAccessException: The requested operation is not allowed` | 应用没有 profile 或未获授权，先确认 workflow 能查询到 profile ID，再按[第 3 步](#3-在-publisher-上授权)授权 |
| `verify` | `tag ... does not match package.json version` | tag 名与 `package.json` 的版本号不一致 |
| `verify` | `tag ... is not on main` | tag 指向的提交不在 `main` 上，先合并再打 tag |
| `verify` | `CHANGELOG.md has no entry` 或 `still marked unreleased` | 补全 changelog 条目后重新打 tag |

`verify` 失败时，先删除远端和本地的 tag，修正后在新的提交上重新打 tag。

## 在本地构建

在 `clients/vscode` 中运行下面的命令。需要 Node.js 22 或更新版本、
仓库指定的 Rust 工具链，以及目标平台的 C 编译器和链接器。Linux 目标静态
链接 musl，需要 `musl-gcc`，在 Debian 和 Ubuntu 上由 `musl-tools` 提供。
在 ARM64 主机上还要设置 `CC_aarch64_unknown_linux_musl=musl-gcc`。

```console
npm ci
npm test
npm run package:pre-release -- --target win32-x64
```

这会生成 `lspf-analysis-win32-x64-0.1.0-pre-release.vsix`，其中包含原生
语言服务器和打包后的客户端。省略 `--target` 时为当前机器构建。打包会替换
共享的 `server/` 目录，因此并行构建不同目标时要使用各自的工作目录。
`npm run package` 构建正式渠道的包。

`package.json` 中的版本号保持 `0.1.0`，`-pre-release` 后缀只出现在文件名中。
脚本会给 `vsce` 传入 `--pre-release`，由它把预发布属性写进 VSIX。manifest
中的 `preview: true` 只添加 Preview 标签，不决定渠道。

## 手动发布

流水线无法发布时，从 workflow 运行的 artifact 或 GitHub release 下载包，
然后在 `clients/vscode` 中上传：

```console
npx vsce publish --azure-credential --skip-duplicate --packagePath <vsix>...
npx ovsx publish --skip-duplicate --packagePath <vsix>... -p <token>
```

使用 `--azure-credential` 前，先用 `az login` 登录一个在 publisher 上具有
Contributor 角色的身份。上传流水线构建好的包，不要重新构建：渠道记录在
VSIX 内部，改文件名不会改变渠道。[publisher 管理页][publishers] 也支持
直接上传 VSIX。

## 版本号规则

每次更新都使用新的数字版本号。Marketplace 的版本号格式是
`major.minor.patch`，不支持 `-beta.1` 之类的后缀。早期预览版使用 `0.1.x`，
第一个正式版使用 `0.2.0`。发布正式版前删除 `preview: true`。预览版和
正式版必须使用不同的版本号。自动更新的行为见[预发布版本的官方说明][pre-release]。

[auth]: https://code.visualstudio.com/api/working-with-extensions/publishing-extension#secure-automated-publishing-to-visual-studio-marketplace
[open-vsx]: https://open-vsx.org/
[publishers]: https://marketplace.visualstudio.com/manage/publishers/
[pre-release]: https://code.visualstudio.com/api/working-with-extensions/publishing-extension#prerelease-extensions
