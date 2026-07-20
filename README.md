# Memo

Tarefas e notas que moram no seu computador, em arquivos que você consegue
abrir sem o app.

Memo é um aplicativo local-first: nada essencial depende de internet ou de
criar conta. Suas tarefas ficam em arquivos Markdown comuns, dentro de uma
pasta que você escolhe — dá pra ler, editar ou versionar com qualquer outra
ferramenta, inclusive o Obsidian. Se um dia você parar de usar o Memo, seus
dados continuam lá, legíveis.

Roda em Linux e Android a partir do mesmo código, com interface nativa
(não é um site empacotado).

> **Status:** em desenvolvimento inicial. Ainda não há versão utilizável.

## Princípios

- **Local first** — funciona inteiro sem internet.
- **Os dados são seus** — formato aberto, acessível fora do app, sem prisão.
- **Simples primeiro** — o que é avançado é opcional e não polui o básico.
- **O que é local é gratuito, pra sempre.** Serviços online (sync, backup)
  são pagos e opcionais, mas nenhum recurso local será removido pra empurrar
  assinatura.
- **Privacidade** — criptografia ponta a ponta nos serviços online.

## Como suas tarefas ficam no disco

```
MeuCaderno/
├── Tarefas/
│   ├── Inbox.md
│   ├── Completas.md
│   └── Compras.md
└── Notas/
```

E dentro de cada arquivo, checklist Markdown comum:

```markdown
- [ ] Comprar leite
- [x] Pagar internet
```

## O que vem por aí

O desenvolvimento é sequencial: cada etapa só começa quando a anterior está
funcionando de verdade.

### Versão 1 — tarefas

- [x] Base do aplicativo rodando no Linux
- [ ] Criar, editar e concluir tarefas, com tudo salvo em arquivos `.md`
- [ ] Listas próprias, além da Inbox padrão, organizadas em pastas
- [ ] Visão de **Hoje** e da **Semana**: você escolhe o que puxar pra cada
      período, em vez de encarar a lista inteira
- [ ] Virada do dia e da semana configurável — inclusive o horário, pra quem
      monta o dia seguinte antes de dormir ou decide de manhã cedo
- [ ] Tarefas concluídas separadas, com desfazer que devolve o item pra
      lista de origem
- [ ] Detecta alterações feitas por fora (útil com Syncthing, Drive etc.)
- [ ] Interface finalizada, com tema claro e escuro
- [ ] Versão Android, com layout adaptado pra toque
- [ ] Pacotes prontos: AppImage/Flatpak no Linux, APK no Android

### Depois da v1

- **Notas** — anotações em Markdown, organizadas em pastas, com busca
- **Recursos opcionais, sempre locais e gratuitos** — kanban, calendário,
  pomodoro, dashboards, ligações entre notas, e suporte a plugins da
  comunidade
- **Serviços online opcionais (pagos)** — sincronização entre dispositivos
  com criptografia ponta a ponta, backup automático com histórico de
  versões, colaboração e publicação de notas

Sincronizar entre dispositivos **hoje já é possível de graça**, apontando
Syncthing, Drive ou similar pra pasta do caderno. O serviço pago é
conveniência, não permissão.

## Desenvolvimento

Requisitos: Rust (stable, via `rustup`), Node.js com npm, e as bibliotecas
de sistema `webkit2gtk-4.1`, `gtk3` e `libsoup3`.

```bash
npm install          # dependências do frontend
npm run tauri dev    # roda o app
cargo test           # testes da lógica de negócio
npm run tauri build  # gera AppImage / deb
```

Estrutura:

| Pasta        | O que é                                                        |
| ------------ | -------------------------------------------------------------- |
| `core/`      | Crate Rust puro com toda a lógica de negócio. Não depende do Tauri. |
| `src-tauri/` | Casca fina que expõe o `core` pro frontend via `invoke()`.      |
| `src/`       | Frontend em Svelte, com CSS puro.                               |
