import{a as M,f as P,s as la}from"../chunks/kP3J42Pu.js";import{K as Pa,x as ia,F as O,y as s,B as e,l as ja,C as a,M as S,g as ca,$ as Ra}from"../chunks/CTLrzvcn.js";import{e as Ba,s as o}from"../chunks/Cyev07QY.js";import{h as Ha,a as t}from"../chunks/d2cDUObl.js";import{a as da,d as L,B as va,s as Ia,b as Na,c as Wa}from"../chunks/CDNtDOHm.js";import{b as pa}from"../chunks/BhNmYaJN.js";/**
 * @license lucide-static v1.33.0 - ISC
 *
 * This source code is licensed under the ISC license.
 * See the LICENSE file in the root directory of this source tree.
 */const c=`
<svg
  class="lucide lucide-check"
  xmlns="http://www.w3.org/2000/svg"
  width="24"
  height="24"
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width="2"
  stroke-linecap="round"
  stroke-linejoin="round"
>
  <path d="M20 6 9 17l-5-5" />
</svg>
`;/**
 * @license lucide-static v1.33.0 - ISC
 *
 * This source code is licensed under the ISC license.
 * See the LICENSE file in the root directory of this source tree.
 */const d=`
<svg
  class="lucide lucide-copy"
  xmlns="http://www.w3.org/2000/svg"
  width="24"
  height="24"
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width="2"
  stroke-linecap="round"
  stroke-linejoin="round"
>
  <rect width="14" height="14" x="8" y="8" rx="2" ry="2" />
  <path d="M4 16c-1.1 0-2-.9-2-2V4c0-1.1.9-2 2-2h10c1.1 0 2 .9 2 2" />
</svg>
`;Pa();var za=P('<meta name="description"/> <meta property="og:title"/> <meta property="og:description"/> <!>',1),Ea=P('<article class="svelte-1uha8ag"><h2 class="svelte-1uha8ag"> </h2> <p class="svelte-1uha8ag"> </p></article>'),Xa=P(`<section class="hero svelte-1uha8ag"><div class="say"><h1 class="svelte-1uha8ag">Nothing survives the call.</h1> <p class="lede svelte-1uha8ag">The OS for agent harnesses. One WebAssembly module, pinned by hash, compiled once,
			and instantiated per invocation.</p> <div class="install code-block svelte-1uha8ag"><span class="file svelte-1uha8ag">crates.io</span> <code class="svelte-1uha8ag">cargo add berm</code> <button class="copy" type="button" aria-label="Copy"><!><!></button></div> <p class="facts svelte-1uha8ag"><span><strong class="svelte-1uha8ag">Apache-2.0</strong> licensed</span> <span>Compiled by <strong class="svelte-1uha8ag">Cranelift</strong></span> <span>Served over <strong class="svelte-1uha8ag">HTTP</strong> and <strong class="svelte-1uha8ag">MCP</strong></span></p></div> <div class="act svelte-1uha8ag"><div class="cta svelte-1uha8ag"><a class="button primary svelte-1uha8ag">Read the docs</a></div></div></section> <section class="call svelte-1uha8ag"><div class="prose svelte-1uha8ag"><h2 class="svelte-1uha8ag">Two results, and they mean different things.</h2> <p class="svelte-1uha8ag">The outer result is the host's — a missing tool, a trap. The inner one is the program
			reporting failure, which is a result the model should see.</p></div> <div class="code-block"><pre><code></code></pre> <button class="copy" type="button" aria-label="Copy code"><!><!></button></div></section> <section class="stage svelte-1uha8ag"><div class="window svelte-1uha8ag"><div class="titlebar svelte-1uha8ag"><span class="light close svelte-1uha8ag" aria-hidden="true"></span> <span class="light minimise svelte-1uha8ag" aria-hidden="true"></span> <span class="light zoom svelte-1uha8ag" aria-hidden="true"></span> <span class="title svelte-1uha8ag">bermd</span></div> <div class="code-block"><pre><code></code></pre> <button class="copy" type="button" aria-label="Copy transcript"><!><!></button></div></div> <p class="note svelte-1uha8ag">A program travels as one OCI layer with no tarball around it, so the digest a registry
		addresses it by is <code>shasum</code> of the file on your disk.</p></section> <section class="pillars svelte-1uha8ag"><div class="grid svelte-1uha8ag"></div></section> <section class="cli svelte-1uha8ag"><div class="column svelte-1uha8ag"><h2 class="svelte-1uha8ag">Running</h2> <p class="svelte-1uha8ag"><code>bermd</code> serves every deployed program's tools over HTTP, and its <code>#[anonymous]</code> tools on one MCP endpoint, named <code></code>.</p> <div class="code-block"><pre><code></code></pre> <button class="copy" type="button" aria-label="Copy commands"><!><!></button></div></div> <div class="column svelte-1uha8ag"><h2 class="svelte-1uha8ag">Moving</h2> <p class="svelte-1uha8ag">A program is one file, so it travels as one OCI layer: <code>berm-oci</code> pushes it to a
			registry, and a host that pulls from one deploys it by reference.</p> <div class="code-block"><pre><code></code></pre> <button class="copy" type="button" aria-label="Copy commands"><!><!></button></div></div></section> <footer class="svelte-1uha8ag"><nav class="left svelte-1uha8ag"><a href="https://github.com/crabtalk" class="svelte-1uha8ag">crabtalk</a> <a class="svelte-1uha8ag">Docs</a></nav> <nav class="right svelte-1uha8ag"><a aria-label="berm on GitHub" class="svelte-1uha8ag"><!></a> <a target="_blank" rel="noreferrer" aria-label="The author on X" class="svelte-1uha8ag"><!></a></nav></footer>`,1);function Va(ha){const ua="berm — the OS for agent harnesses",ga=`<span class="mark">let</span> berm = Berm::new(&amp;engine, calls, vec![], storage::Memory::new());
berm.deploy(<span class="s">"example"</span>, &amp;wasm)?;

<span class="mark">match</span> berm.call(None, <span class="s">"example"</span>, <span class="s">"echo"</span>, <span class="s">br#"{"query":"hello"}"#</span>.to_vec()).<span class="mark">await</span>?.result {
    Ok(result) =&gt; println!(<span class="s">"{result}"</span>),
    Err(failure) =&gt; eprintln!(<span class="s">"{failure}"</span>),
}`,j="222890c498ed28f4bf60670a223141489d9879020bd1890111b8c11ac79fa31d",ma=`<span class="c">$</span> curl -s localhost:7777/programs/fixture/idl | jq -r .digest
<span class="mark">${j}</span>

<span class="c">$</span> shasum -a256 ./fixture.wasm
<span class="mark">${j}</span>  ./fixture.wasm`,ba=`<span class="c">$</span> bermd &amp;
<span class="c">$</span> curl -X PUT --data-binary @example.wasm <span class="s">'localhost:7777/programs/example?public=echo'</span>
<span class="c">$</span> curl localhost:7777/programs`,ya=`<span class="mark">let</span> registry = Registry::open(&amp;reference, Access::Write)?;
registry.push(<span class="s">"v1"</span>, &amp;wasm, &amp;manifest)?;`,fa=[{title:"Pinned by hash",body:"A program is one WebAssembly module. berm compiles it once and instantiates it per invocation — arguments go in through syscalls, the result comes back out of guest memory, and the instance is gone."},{title:"The linker is the boundary",body:"A program reaches the world only through the syscalls it was given, and that list is the Linker it was instantiated with. A call to anything else traps because nothing is registered for it, not because a check said no."},{title:"berm has no host",body:"berm serves calls between programs and the facts of a call — its caller, its user, the clock. State, HTTP, a database and wakes are decisions about a host, so berm builds the syscalls and leaves what they act on to whoever is building one."},{title:"Read it without running it",body:"Manifest::from_image reads what an image claims to be — its tools, their schemas, when to reach for them — without compiling it and without running it."}],_a=`<script type="application/ld+json">${JSON.stringify({"@context":"https://schema.org","@type":"SoftwareSourceCode",name:"berm",description:L,codeRepository:da,programmingLanguage:"Rust",license:"https://www.apache.org/licenses/LICENSE-2.0",keywords:["WebAssembly sandbox","wasmtime","MCP server","agent tools","LLM tool sandbox","OCI artifact","Rust sandbox"]})}<\/script>`;var R=Xa();Ha("1uha8ag",l=>{var i=za(),r=ia(i),n=s(r,2);o(n,"content",ua);var p=s(n,2),h=s(p,2);t(h,()=>_a),O(()=>{o(r,"content",L),o(p,"content",L)}),ja(()=>{Ra.title="berm — the OS for agent harnesses"}),M(l,i)});var u=ia(R),g=e(u),B=s(e(g),4),H=s(e(B),4),I=e(H);t(I,()=>d);var wa=s(I);t(wa,()=>c),a(H),a(B),S(2),a(g);var N=s(g,2),W=e(N),ka=e(W);a(W),a(N),a(u);var m=s(u,2),z=s(e(m),2),b=e(z),E=e(b);t(E,()=>ga,!0),a(E),a(b);var X=s(b,2),q=e(X);t(q,()=>d);var xa=s(q);t(xa,()=>c),a(X),a(z),a(m);var y=s(m,2),G=e(y),D=s(e(G),2),f=e(D),F=e(f);t(F,()=>ma,!0),a(F),a(f);var J=s(f,2),K=e(J);t(K,()=>d);var Ca=s(K);t(Ca,()=>c),a(J),a(D),a(G),S(2),a(y);var _=s(y,2),U=e(_);Ba(U,5,()=>fa,l=>l.title,(l,i)=>{var r=Ea(),n=e(r),p=e(n,!0);a(n);var h=s(n,2),La=e(h,!0);a(h),a(r),O(()=>{la(p,ca(i).title),la(La,ca(i).body)}),M(l,r)}),a(U),a(_);var w=s(_,2),k=e(w),x=s(e(k),2),Ta=s(e(x),4);Ta.textContent="{program}.{tool}",S(),a(x);var V=s(x,2),C=e(V),Q=e(C);t(Q,()=>ba,!0),a(Q),a(C);var Y=s(C,2),Z=e(Y);t(Z,()=>d);var $a=s(Z);t($a,()=>c),a(Y),a(V),a(k);var aa=s(k,2),ea=s(e(aa),4),T=e(ea),sa=e(T);t(sa,()=>ya,!0),a(sa),a(T);var ta=s(T,2),oa=e(ta);t(oa,()=>d);var Aa=s(oa);t(Aa,()=>c),a(ta),a(ea),a(aa),a(w);var ra=s(w,2),$=e(ra),Ma=s(e($),2);a($);var na=s($,2),v=e(na),Oa=e(v);va(Oa,{get icon(){return Ia},size:15}),a(v);var A=s(v,2),Sa=e(A);va(Sa,{get icon(){return Na},size:14}),a(A),a(na),a(ra),O(()=>{o(ka,"href",`${pa??""}/docs/`),o(Ma,"href",`${pa??""}/docs/`),o(v,"href",da),o(A,"href",Wa)}),M(ha,R)}export{Va as component};
