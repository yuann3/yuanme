
---
title: "Automorfismos de corpos e o grupo de Galois"
number-sections: true
lang: pt-BR
---

## Automorfismos de corpos


:::{#def-field-aut}
Seja $\E$ um corpo. Lembre que um mapa $\varphi:\E\to\E$ bijetivo é dito automorfismo de $\E$ se $\varphi(x+y)=\varphi(x)+\varphi(y)$ e $\varphi(xy)=\varphi(x)\varphi(y)$ vale para todo $x,y\in\E$. O conjunto de automorfismos de $\E$ é um grupo com a operação de composição e este grupo é denotado por $\aut \E$. Se $\varphi\in\aut\E$, então definimos
$$
\fix\varphi=\{x\in \E\mid \varphi(x)=x\}.
$$
:::

:::{#exr-field-aut}
$\fix\varphi$ é um corpo, portanto $\E:\fix\varphi$ é uma extensão de corpos.
:::

:::{#exr-field-aut}
A conjugação complexa $\varphi: x+iy\mapsto x-iy$ é um automorfismo de  $\C$ e  $\fix\varphi = \R$.
:::

:::{#exm-field-aut}
Seja $p$ um primo e seja $f(x)\in\Z_p[x]$ um polinômio irredutível de grau $d$. Defina $\E=\Z_p[x]/(f(x))$ e note que $\E$ é um corpo de $q=p^d$ elementos. 
Pelo @exr-charp, o mapa $\varphi:\E\to\E$ definido como $\alpha\mapsto \alpha^p$ é um automorfismo. 
<!--É bem conhecido que $(\alpha+\beta)^p=\alpha^p+\beta^p$ e $(\alpha\beta)^p=\alpha^p\beta^p$, então precisamos verificar apenas que $\varphi$ é uma bijeção. Como $\E$ é um corpo finito, é suficiente verificar que $\varphi$ é injetivo. Isso é verdade, pois se $\alpha,\beta\in\E$ tais que $\varphi(\alpha)=\varphi(\beta)$, então
$$
0=\alpha^p-\beta^p=(\alpha-\beta)^p
$$
que implica que $\alpha-\beta=0$; ou seja $\alpha=\beta$. Temos então que $\varphi\in\aut\E$. 
-->
Além disso, $\fix\varphi$ é o conjunto de soluções da equação $x^p=x$ e esta equação tem precisamente $p$ soluções, nomeadamente, os elementos de $\Z_p$. Logo $\fix\varphi=\Z_p$. O automorfismo $\varphi$ é conhecido como o automorfismo de <a href="https://pt.wikipedia.org/wiki/Ferdinand_Georg_Frobenius">Frobenius</a>.
:::

## O grupo de Galois

:::{#def-galois-group}
Seja $\E:\F$ uma extensão de corpos. Defina
$$
\gal \E\F=\{\alpha\in\aut\E\mid \F\subseteq\fix\alpha\}.
$$
Em outras palavras, $\gal\E\F$ é o conjunto de automorfismos $\varphi$ de $\E$ tais que $\varphi(x)=x$ para todo $x\in\F$. É fácil verificar que $\gal\E\F$ é um subgrupo de $\aut\E$. Este subgrupo é chamado de grupo de <a href="https://pt.wikipedia.org/wiki/%C3%89variste_Galois">Galois</a> da extensão $\E:\F$ ou de grupo de Galois de $\E$ sobre $\F$.
:::

:::{#exr-gal-group}
Se $\varphi\in\gal\E\F$, então $\varphi$ é uma transformação $\F$-linear de $\E$.
:::

:::{#lem-aut-pol}
Seja $\E:\F$ uma extensão, $f(x)\in\F[x]$, $\alpha\in\E$ tal que $f(\alpha)=0$, e $\varphi\in\gal\E\F$. Então $f(\varphi(\alpha))=0$. Em particular, $\gal\E\F$ age (veja @def-action) sobre o conjunto das raízes de $f(x)$ em $\E$
:::

:::{.proof}
Exercício. Revise a demonstração do fato bem conhecido que se um número complexo é raiz de um polinômio com coeficientes reais, então o conjugado complexo deste número será também raiz do mesmo polinômio (@lem-comp-conj-root).
:::

:::{#exm-gal}
Determinemos $\gal\C\R$. Seja $\varphi\in\gal\C\R$. Como $\varphi$ é uma transformação $\R$-linear, as imagens $\varphi(1)$ e $\varphi(i)$ determinam $\varphi$. Como $1\in\R$, temos que $\varphi(1)=1$. Note que $i$ é raiz do polinômio $x^2+1$, então $\varphi(i)$ será raiz do mesmo polinômio que implica que $\varphi(i)=\pm i$. Escolhendo $\varphi(i)=i$, obtemos que $\varphi=\mbox{id}_{\C}$, enquanto escolhendo $\varphi(i)=-i$, obtemos a conjugação complexa. Obtivemos então que $\gal\C\R$ é um grupo de ordem 2.
:::

:::{#exm-gal2}
Seja $\xi=\exp(2\pi i/3)$ (terceira raiz da unidade) e considere a extensão $\E=\Q(\xi)$ de $\Q$. Note que $\xi$ é raiz do polinômio $x^3-1=(x-1)(x^2+x+1)$ e que o polinômio $x^2+x+1$ é irredutível sobre $\Q$. Em particular, $m_\xi(x)=x^2+x+1$ e $\E$ é um espaço vetorial de dimensão $2$ sobre $\Q$. Além disso,
$$
\E=\left<1,\xi\right>_{\Q}=\{\alpha+\beta\xi\mid \alpha,\beta\in\Q\}
$$
e para determinar os automorfismos $\varphi\in\gal\E\Q$, é suficiente saber $\varphi(1)$ e $\varphi(\xi)$. Como no exemplo, anterior, $\varphi(1)=1$ e $\varphi(\xi)$ é raiz de $x^2+x+1$; consequentemente $\varphi(\xi)=\xi$ ou $\varphi(\xi)=\xi^2=\bar\xi$ (conjugado complexo). O primeiro caso dá o automorfismo identidade, enquanto o segundo dá o automorfismo
$$
\alpha+\beta\xi\mapsto \alpha+\beta\bar\xi^2 \quad\mbox{onde}\quad\alpha,\beta\in\Q.
$$
(Na verdade, precisa verificar que isso é um automorfismo, mas isso é uma conta simples que o leitor pode fazer.) Então $|\gal\E\Q|=2$.
:::

:::{#exm-sqroot-3}
Seja $\alpha=\sqrt[3]2$ e considere o corpo $\Q(\alpha)$. Pelo Critério de Einsenstein (@thm-eisenstein), o polinômio $x^3-2\in\Q[x]$ é irredutível, $\alpha$ é  raiz deste polinômio, e assim @lem-pol-min implica que $m_\alpha(x)=x^3-2$. Obtemos por @lem-Fa que 
$$
\Q(\alpha)\cong\Q[x]/(x^3-2).
$$
Em particular, $\Q(\alpha):\Q$ é uma extensão de grau $3$. 

Observe que o corpo $\Q(\alpha)$ não é corpo de decomposição de $m_\alpha(x)=x^3-2$! De fato, as raízes complexas de $x^3-2$ são 
$$
\alpha=\sqrt[3]2,\quad \alpha_1=\sqrt[3]2\xi_1,\quad \alpha_2=\sqrt[3]2\xi_2
$$
onde 
$$
\xi_1=\exp(2\pi i/3)\quad\mbox{e}\quad \xi_2=\exp(4\pi i/3)=\xi_1^2
$$
são as terceiras raízes não triviais da unidade. Por definição, o corpo de decomposição de $x^3-2$ é 
$$
\E=\Q\left(\sqrt[3]2,\sqrt[3]2\xi_1,\sqrt[3]2\xi_2\right)=\Q\left(\sqrt[3]2,\sqrt[3]2\xi_1,\sqrt[3]2\xi_1^2\right).
$$
Notando que $\xi_1=\alpha_1/\alpha\in\E$, obtemos que 
$$
\E=\Q\left(\sqrt[3]2,\xi_1\right)=\Q\left(\sqrt[3]2\right)(\xi_1).
$$
O polinômio minimal de $\xi_1$ sobre $\Q\left(\sqrt[3]2\right)$ é $x^2+x+1$, e assim 
$$
\E=\Q(\sqrt[3]2)[x]/(x^2+x+1).
$$
Temos que a extensão $\E:\Q$ é de grau $6$.

Afirmamos que $\gal{\Q(\sqrt[3]2)}\Q$ é trivial. De fato, se $g\in\gal{\Q(\sqrt[3]2)}\Q$, então $g$ está determinado por $g(\sqrt[3]2)$. 
Por outro lado, $m_{\sqrt[3]2}(x)=x^3-2$, e @lem-aut-pol implica que $g(\sqrt[3]2)$ é raiz do polinômio $x^3-2$. As demais raízes deste polinômio não são reais, mas $\Q(\sqrt[3]2)\subseteq\R$. Portanto, $g(\sqrt[3]2)=\sqrt[3]2$ e $g=\mbox{id}_{\Q(\sqrt[3]2)}$. 
Logo $\gal{\Q(\sqrt[3]2)}\Q=1$. 
:::

:::{#exr-auts-li}
Seja $\E$ um corpo e sejam $\varphi_1,\ldots,\varphi_k\in\aut\E$ distintos. Assuma que $\alpha_1,\ldots,\alpha_k\in\E$ são  tais que $\alpha_1\varphi_1+\cdots+\alpha_k\varphi_k=0$. Mostre que $\alpha_1=\cdots=\alpha_k=0$. Em outras palavras, $\varphi_1,\ldots,\varphi_k$ são linearmente independentes no $\E$-espaço 
$$
\mbox{Func}(\E,\E)=\{f:\E\to\E \mbox{ é uma função}\}.
$$
:::

:::{#lem-gal-ff}
Seja $\E:\F$ uma extensão finita. Então $|\gal\E\F|\leq \dim_{\F}\E$.
:::

:::{.proof}
Os elementos de $\gal\E\F$ são elementos do $\E$-espaço $\mbox{Hom}_{\F}(\E,\E)$ (@exr-gal-group). Dada uma base $B$ de $\E$ sobre $\F$, $\mbox{Hom}_{\F}(\E,\E)$ é isomorfo como 
$\E$-espaço com 
$$
\mbox{Func}(B,\E)=\{f:B\to \E\mid f\mbox{ é função}\}.
$$
Note que $\dim_{\E}\mbox{Func}(B,\E)=|X|=\dim_{\F}\E$. Portanto, $n+1$ elementos distintos $\varphi_1,\ldots,\varphi_{n+1}\in\gal\E\F$ 
são linearmente dependentes em $\mbox{Func}(B,\E)$ e eles são também linearmente dependentes em $\mbox{Hom}_{\F}(\E,\E)$. 
Ora, @exr-auts-li implica que 
$$
|\gal\E\F|leq\dim_{\E}\mbox{Hom}_{\F}(\E,\E)=\dim_{\E}\mbox{Func}(B,\E)=|B|=\dim_{\F}\E.
$$
<!--são elementos distintos de $\gal\E\F$. Vamos obter uma contradição. Seja $e_1,\ldots,e_n$ uma base de $\E$ sobre $\F$. Considere o sistema de equações lineares sobre $\E$:
\begin{eqnarray*}
\varphi_1(e_1)x_1+\cdots+\varphi_{n+1}(e_1)x_{n+1}&=&0\\
&\vdots&\\
\varphi_1(e_n)x_1+\cdots+\varphi_{n+1}(e_n)x_{n+1}&=&0.
\end{eqnarray*}
Como temos $n$ equações para $n+1$ incógnitas, existe uma solução não trivial $(\alpha_1,\ldots,\alpha_{n+1})\in\E^{n+1}$ e obtemos usando esta solução que
$$
0=\alpha_1\varphi_1(e_i)+\cdots+\alpha_{n+1}\varphi_{n+1}(e_i)=(\alpha_1\varphi_1+\cdots+\alpha_{n+1}\varphi_{n+1})(e_i)
$$
para todo $i\in\{1,\ldots,n\}$.
Isso implica que $\alpha_1\varphi_1+\cdots,\alpha_{n+1}\varphi_{n+1}=0$, mas isso é impossível pelo exercício anterior.-->
:::

:::{exm-gal-ff}
Nos @exm-gal e @exm-gal2, $|\gal\E\F|= \dim_{\F}\E$. No entanto, no @exm-sqroot-3 temos desigualdade própria $|\gal\E\F|< \dim_{\F}\E$. O motivo disso 
vai ser explicado pelo conceito de extensões normais a ser discutido na seção seguinte.
:::

## Extensões normais

:::{#def-normal-extension}
Uma extensão $\E:\F$ algébrica é dita *normal* se para todo polinômio irredutível $f(x)\in\F[x]$, se $f(x)$ possui raiz em $\E$, então $f(x)$ decompõe-se em produto de polinômios do primeiro grau (ou seja, $\E$ contem um corpo de decomposição para $f(x)$).
:::

:::{#exm-normal-cuberoot-3}
Pelo @exm-sqroot-3, a extensão $\Q(\sqrt[3]2):\Q$ não é normal.
:::

:::{#lem-normal-splitting}
Assuma que $\E:\F$ é uma extensão finita. Então ela é normal se e somente se $\E$ é o corpo de decomposição de algum polinômio $f(x)\in\F[x]$. 
:::
:::{.proof}
Assuma primeiro que $\E:\F$ é finita e é normal. Então $\E=\F(\alpha_1,\ldots,\alpha_k)$ com $\alpha_i\in\E$. Note que $m_{\alpha_i}(x)$ possui raiz em $\E$ (nomeadamente, $\alpha_i$) e ele decompõe-se em produto de polinômios do primeiro grau. Portanto, 
$$
f(x)=m_{\alpha_1}(x)\cdots m_{\alpha_k}(x)
$$
também decompõe-se em produto de polinômios do primeiro grau em $\E$. Por outro lado, $\E$ é gerado pelas raízes de $f(x)$, então $\E$ é corpo de decomposição de $f(x)$ sobre $\F$. 

Assuma agora que $\E$ é corpo de decomposição de algum polinômio $f(x)$. Seja $m(x)\in\F[x]$ um polinômio irredutível tal que 
$m(x)$ possui raiz $\alpha$ em $\E$. Seja $\K$ o corpo de decomposição de $m(x)$ sobre $\E$ e assuma que $\beta\in\K$ é uma outra raiz de $m(x)$. Basta provar que $\beta\in\E$. Considere as cadeias
$$
\E(\alpha):\F(\alpha):\F\quad\mbox{e}\quad \E(\beta):\F(\beta):\F.
$$
As extensões $\F(\alpha):\F$ e $\F(\beta):\F$ são isomorfas, pois $m_\alpha(x)=m_\beta(x)=m(x)\in\F[x]$. Além disso, 
$\E(\alpha)$ é corpo de decomposição de $f(x)$ sobre $\F(\alpha)$ e $\E(\beta)$ é corpo de decomposição de $f(x)$ sobre $\F(\beta)$. Portanto, 
as extensões $\E(\alpha):\F(\alpha)$ e $\E(\beta):\F(\beta)$ são também isomorfas. Em particular, 
$$
\dim_{\F}\E(\alpha)=\dim_{\F}\F(\alpha)\cdot\dim_{\F(\alpha)}\E(\alpha)=\dim_{\F}\F(\beta)\cdot\dim_{\F(\beta)}\E(\beta)=\dim_{\F} \E(\beta).
$$
Por outro lado, $\alpha\in\E$ e $\E(\alpha)=\E$ e obtemos que 
$$
\dim_{\F}\E=\dim_{\F} \E(\beta).
$$
que (considerando que as dimensões são finitas) implica que $\E(\beta)=\E$; ou seja, $\beta\in\E$ com foi o objetivo deste argumento.
:::

:::{#lem-lifting-isom}
[Lema da Extensão]
Seja $\E:\F$ uma extensão normal e finita e $\K_1,\K_2\subseteq \E$ corpos tais que $\F\subseteq \K_1,\K_2$. Assuma que $\psi:\K_1\to \K_2$ um isomorfismo tal que $\psi|_{\F}=\mbox{id}_{\F}$. Então existe $\bar\psi\in\gal\E\F$ tal que $\bar\psi|_{\K_1}=\psi$. Em outras palavras, o isomorfismo $\psi$ pode ser extendido a um elemento de $\gal\E\F$.    
:::
:::{.proof}
Por @lem-normal-splitting, $\E$ é corpo de decomposição de $f(x)\in\F[x]$. Pela definição do corpo de decomposição (@def-decompfield), $\E$ é corpo de decomposição de $f(x)$ sobre $\K_1$ e sobre $\K_2$. O resto segue do @thm-decompfield-unique.
:::

## Extensões separáveis

:::{#exr-derivado}
Seja $f(x)\in\F[x]$ um polinômio. Mostre que as seguintes são equivalentes:

1. $f(x)$ não possui raiz múltipla em nenhuma extensão de $\F$.
2. $\mbox{mdc}(f(x),f'(x))=1$ onde $f'(x)$ é o derivado de $f(x)$. 
:::

:::{#def-sep-ext}
Seja $\F$ um corpo. Um polinômio $f(x)\in\F[x]$ chama-se *separável* se $f(x)$ não possui raízes múltiplas em nenhuma extensão de $\F$. 
Seja $\E:\F$ uma extensão algébrica. 
 Um elemento $\alpha\in\E$ é dita *separável* (sobre $\F$), se $m_\alpha(x)$ é um polinômio separável. 
 A extensão $\E:\F$ é *separável* se todo elemento $\alpha\in\E$ é separável.
:::

Note que a separabilidade de $f(x)$ e de $m_\alpha(x)$ no @def-sep-ext pode ser verificada usando o critério no @exr-derivado.

:::{#exr-sep-ext}
Seja $\E:\F$ uma extensão algébrica. Mostre que ela é separável se 

1. a caraterística de $\F$ (e de $\E$) é zero;
2. $\E$ é finito.
:::


:::{#thm-transitive}
Seja $\F$ um corpo. Assuma que $\E$ é um corpo de decomposição de um polinômio irredutível $f(x)\in\F[x]$ com $\mbox{grau}\,f(x)=k$. Então $G=\gal\E\F$ permuta as raízes de $f(x)$ transitivamente. Além disso, se $f(x)$ é  separável, então $G$ pode ser visto como um subgrupo transitivo de $S_k$ e $k$ divide $|G|$.
:::

:::{.proof}
Sejam $\alpha,\beta\in\E$ duas raízes de $f(x)$. Note que $m_\alpha(x)=m_\beta(x)=f(x)$. O @cor-simple implica que existe um isomorfismo $\psi:\F(\alpha)\to\F(\beta)$ tal que $\psi|_{\F}=\mbox{id}_{\F}$ e $\psi(\alpha)=\beta$. Pelo @lem-lifting-isom, existe $\varphi\in\gal\E\F$ tal que $\varphi(\alpha)=\beta$. Isso implica que $\gal\E\F$ permuta transitivamente as raízes de $f(x)$.

Ora assuma que $f(x)$ é separável. Para mostrar que $\gal\E\F$ pode ser considerado como um subgrupo de $S_k$, observe que $f(x)$ não possui raízes múltiplas em $\E$ (pois $f(x)$ é separável) e isso significa que $f(x)$ tem precisamente $k$ raízes, nomeadamente $\alpha_1,\ldots,\alpha_k$ mutualmente distintas. Se um elemento $\sigma$ de $\gal\E\F$ está no núcleo da ação em $\{ \alpha_1,\ldots,\alpha_k\}$, então $\sigma$ fixa todas as raízes. Mas como $\E$ é gerado por estas raízes (sendo corpo de decomposição), $\sigma=\mbox{id}_{\E}$. Então $\gal\E\F$ pode ser visto como um subgrupo do grupo simétrico do conjunto  $\{ \alpha_1,\ldots,\alpha_k\}$ e este grupo simétrico é claramente isomorfo a $S_k$. Finalmente, o fato que $k\mid |\gal\E\F|$ segue do Teorema Órbita-Estabilizador (@cor-orbit-stab).
:::



:::{#lem-simple-ext}
Seja $\E:\F$ uma extensão finita e separável. Então existe um elemento $\gamma\in\E$ tal que $\E=\F(\gamma)$. Ou seja, $\E:\F$ é uma extensão simples.
:::

:::{.proof}
Primeiro, se $\F$ é um corpo finito, então $\E$ (extensão finita de $\F$) é também finito e $\E=\F(\alpha)$ onde $\alpha\in\E$ é um gerador de $\E^*=\E\setminus\{0\}$ (@thm-field-cyclic). Ou seja, $\E$ é extensão simples e podemos assumir no resto do argumento que $\F$ é corpo infinito. 

É suficiente provar que quando $\E=\F(\alpha,\beta)$, existe $\gamma\in\E$ tal que $\E=\F(\gamma)$. Afirmamos que existe algum $c\in\F$ tal que $\E=\F(\alpha,\beta)=\F(\alpha+c\beta)$. Seja $c\in\F$ tal que $\F(\alpha,\beta)\neq\F(\alpha+c\beta)$ e ponha $\K=\F(\alpha+c\beta)$. Por suposição, $\K\subset \F(\alpha,\beta)$ e $\alpha,\beta\not\in\K$. Seja $\LL$ um corpo de decomposição de 
$m_\beta(x)\in\K[x]$ sobre $\K$. Note que $m_\beta(x)$ é divisor do polinômio minimal $m_\beta^{\F}(x)$ de $\beta$ sobre $\F$. Por suposição $m_\beta^{\F}(x)$ é um polinômio separável, então $m_\alpha(x)$ também é separável. Além disso, $\alpha=(\alpha+c\beta)-c\beta\in\LL$ e portanto $\E\subseteq \LL$. 
Como $\mbox{grau}\,m_\beta(x)$ é maior que dois e $m_\beta(x)$ é separável, existe $\beta'\in\LL\setminus\{\beta\}$ tais que $m_\beta(\beta')=0$. Pelo @thm-transitive, existe $\sigma$ um automorfismo em $\gal{\LL}{\K}$ tal que $\sigma(\beta)=\beta'$ e $\sigma(\alpha+c\beta)=\alpha+c\beta$. Portanto 
$$
\alpha+c\beta=\sigma(\alpha+c\beta)=\sigma(\alpha)+c\sigma(\beta)=\sigma(\alpha)+c\beta'.
$$ 
e
$$
c = \frac{\alpha-\sigma(\alpha)}{\beta'-\beta}.
$$
Como $\sigma\in \gal{\LL}{\K}$, temos por @lem-gal-ff, temos um número finito de possibilidades para a expressão no lado direito da equação anterior. Isso quer dizer que temos apenas um número finito de possibilidades para $c\in\F$ tal $c$ satisfaz a equação anterior. Seja $c\in\F$ tal que $c$ não satisfaz esta 
equação. Temos com este $c$ que $\F(\alpha+c\beta)= \F(\alpha,\beta)$.
:::

## Extensões de Galois

:::{#def-ext-gal}
Uma extensão $\E:\F$ é dita *extensão de Galois* se ela é algébrica, normal e separável.
:::

:::{#thm-gg-fix-normal}
Seja $\E:\F$ uma extensão de Galois finita. Temos que 
$$
\fix{\gal\E\F}=\{\alpha\in\E\mid g(\alpha)=\alpha\mbox{ para todo }g\in\gal\E\F\}=\F.
$$
:::
:::{.proof}
Claramente $\F\subseteq \fix{\gal\E\F}$ então precisa provar a outra inclusão. Assuma que $\K=\fix{\gal\E\F}$ e seja $\alpha\in \E\setminus\F$. 
Vamos provar que $\alpha\not\in\K$. Seja $m_\alpha(x)$ o polinômio mínimo de $\alpha$ sobre $\F$ e note que o grau de $m_\alpha(x)$ é maior ou igual a $2$. Como a extensão 
$\E:\F$ é  normal e separável, $m_\alpha(x)$ é produto de fatores do primeiro grau em $\E$ e existe $\beta\in\E$ diferente de $\alpha$ tal que 
$m_\alpha(\beta)=0$. Ora, existe um isomorfismo $\psi:\F(\alpha)\to \F(\beta)$ tal que $\psi|_{\F}=\mbox{id}_{\F}$. Pelo @lem-lifting-isom, existe um automorfismo $\varphi\in\gal\E\F$ tal que $\varphi(\alpha)=\beta$. Isso implica que $\alpha\not\in\fix{\gal\E\F}$. 
:::

:::{#lem-H-pol}
Seja $\E$ um corpo e seja $H\leq\aut\E$ um subgrupo finito. Assuma que $\alpha\in\E$. Considere o polinômio 
$$
f_{H,\alpha}(x)=\prod_{h\in H}(x-h(\alpha)).
$$ 
Denotando por $\fix H$ or corpo 
$$
\fix H=\{\beta\in\E\mid h(\beta)=\beta\mbox{ para todo }h\in H\},
$$
temos que 

1. $f_{H,\alpha}(x)\in\fix H[x]$;
2. $f_{H,\alpha}(\alpha)=0$; 
3. $\mbox{grau}f_{H,\alpha}(x)=|H|$. 

Em particular, $\alpha$ é algébrico sobre $\fix H$ e $\dim_{\fix H}\fix H(\alpha)\leq |H|$. 
:::
:::{.proof}
Exercício.
:::

:::{#thm-gg-order}
Seja $\E:\F$ uma extensão de Galois finita. Então $|\gal\E\F|=\dim_{\F}\E$. 
:::
:::{.proof}
Seja $n=|\gal\E\F|$. Pelo  @lem-gal-ff, $n\leq\dim_{\F}\E$, então basta provar a outra desigualdade. Pelo @lem-simple-ext, temos que $\E=\F(\alpha)$ com algum $\alpha\in\E$. Afirmamos que $\mbox{grau}\,m_\alpha(x)\leq n$ e isso vai implicar que $\dim_{\F}\E\leq n$. Considere o polinômio $f_{H,\alpha}(x)$ no @lem-H-pol. Temos que $f_{H,\alpha}(x)\in\fix H[x]=\F[x]$, 
$f_{H,\alpha}(\alpha)=0$ e  $\mbox{grau}f_{H,\alpha}(x)=|H|$. Portanto, 
$$
\dim_{\F}\E=\mbox{grau}\,m_\alpha(x)\leq \mbox{grau} f_{H,\alpha}(x)=n=|H|.
$$
:::

:::{#exm-finite-field-galois}
Considere um corpo $\F_q$ com $q$ elementos onde $q=p^d$ com algum primo $p$. Como $\F_q$ é corpo de decomposição do polinômio $x^q-x\in\Z_p[x]$ (@thm-finite-field),  temos que a extensão $\F_q:\Z_p$ é normal. Portanto, $|\gal{\F_q}{\Z_p}|=\dim_{\Z_p}\F_q=d$. De fato, $\gal{\F_q}{\Z_p}$ é cíclico gerado pelo automorfismo de Frobenius $\varphi:x\mapsto x^p$.  
:::

:::{#lem-normal-invariant}
Seja $\E:\K:\F$ uma cadeia de extensões e assuma que $\E:\F$ é extensão de Galois finita. 

1. $\E:\K$ é de Galois finita. 
2. $\K:\F$ é normal se e somente se $\sigma(\K)=\K$ para todo $\sigma\in\gal\E\F$. Neste caso, $\K:\F$ é Galois. 
:::
:::{.proof}
1. Por @lem-normal-splitting, $\E$ é corpo de decomposição de $f(x)\in\F[x]$ sobre $\F$. Ora, $\E$ é corpo de decomposição do mesmo $f(x)$ sobre $\K$. Logo $\E:\K$ é normal e finita. Precisa provar que $\E:\K$ é extensão separável. Seja $\alpha\in\E$ e considere o polinômio minimal $m_\alpha(x)\in\K[x]$.
Note que $m_\alpha(x)$ é divisor do polinômio minimal $p(x)\in\F[x]$ de $\alpha$ sobre $\F$. Como $\E:\F$ é separável, $p(x)$ não possui raízes múltiplas em nenhuma extensão e assim $m_\alpha(x)$ também não possui raízes múltiplas em nenhuma extensão.

2. Assuma que $\K:\F$ é normal. Seja $\sigma\in\gal\E\F$ e $\alpha\in\K$. Seja $m_\alpha(x)\in\F[x]$ o polinômio minimal de $\alpha$. Por @lem-aut-pol, 
$\sigma(\alpha)$ é raiz de $m_\alpha(x)$. Como $\K:\F$ é normal, temos que $\sigma(\alpha)\in \K$ e assim $\K\subseteq \sigma(\K)$. Como $\sigma$ é um $\F$ isomorfismo e $\dim_{\F}\K$ é finita, obtemos que  $\K= \sigma(\K)$. Ora assuma que $\sigma(\K)=\K$ para todo $\sigma\in\gal\E\F$. Seja $q(x)\in\F[x]$ 
um polinômio irredutível que possui raiz $\alpha\in\K$. Seja $\beta\in\E$ outra raiz de $q(x)$. Por @lem-Fa, existe $\psi:\F(\alpha)\to \F(\beta)$ 
isomorfismo tal que $\psi|_{\F}=\mbox{id}_{\F}$ e $\psi(\alpha)=\psi(\beta)$. Pelo @lem-lifting-isom, existe um $\sigma\in\gal\E\F$ tal que $\sigma|_{\F(\alpha)}=\psi$. Portanto, $\beta\in\sigma(\K)=\K$, e temos que toda raiz de $p(x)$ está contida em $\K$. Logo $\K$ é extensão normal. 
:::