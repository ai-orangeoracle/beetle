(function(){
  var G=typeof window!=='undefined'?window:typeof self!=='undefined'?self:this;
  var BASE='';
  var csrfToken=null;
  var T={
    zh:{
      pairing_title:'配对码设置', pairing_h2:'设置 6 位配对码', pairing_desc:'首次使用请设置配对码，后续执行保存、重启等操作时需输入此码。',
      pairing_label:'配对码（6 位数字）', pairing_ph:'请输入 6 位数字', pairing_btn:'确认设置',
      pairing_msg_6digits:'请输入 6 位数字', pairing_fail:'设置失败', pairing_network:'网络错误',
      modal_enter_code:'请输入配对码', modal_ph:'6 位数字', modal_ok:'确认', modal_cancel:'取消',
      nav_wifi:'WiFi 配置',
      wifi_title:'WiFi 配置', wifi_h2:'连接 WiFi', wifi_ssid_label:'WiFi 名称 (SSID)', wifi_ssid_ph:'留空则仅使用设备热点',
      wifi_pass_label:'WiFi 密码', wifi_pass_ph:'开放网络可留空', wifi_save:'保存', wifi_restart:'重启设备',
      wifi_saved_msg:'保存成功，重启后生效。', wifi_save_fail:'保存失败', wifi_restarting:'设备重启中…', wifi_restart_fail:'重启请求失败'
    },
    en:{
      pairing_title:'Pairing', pairing_h2:'Set 6-digit pairing code', pairing_desc:'Set a pairing code for the first time. You will need it for save, restart, etc.',
      pairing_label:'Pairing code (6 digits)', pairing_ph:'Enter 6 digits', pairing_btn:'Set',
      pairing_msg_6digits:'Please enter 6 digits', pairing_fail:'Failed to set', pairing_network:'Network error',
      modal_enter_code:'Enter pairing code', modal_ph:'6 digits', modal_ok:'OK', modal_cancel:'Cancel',
      nav_wifi:'WiFi',
      wifi_title:'WiFi', wifi_h2:'Connect WiFi', wifi_ssid_label:'WiFi name (SSID)', wifi_ssid_ph:'Leave blank to use device AP only',
      wifi_pass_label:'Password', wifi_pass_ph:'Leave blank for open network', wifi_save:'Save', wifi_restart:'Restart device',
      wifi_saved_msg:'Save successful, will take effect after restart.', wifi_save_fail:'Save failed', wifi_restarting:'Restarting…', wifi_restart_fail:'Restart failed'
    }
  };
  function showMsg(el,text,isErr){
    var e=typeof el==='string'?document.getElementById(el):el;
    if(!e)return;
    e.textContent=text;
    e.className='msg '+(isErr?'err':'ok');
    e.style.display=text?'block':'none';
  }
  function createPairingModal(){
    if(document.getElementById('modalPairing'))return;
    var wrap=document.createElement('div');
    wrap.id='modalPairing';
    wrap.className='modal';
    wrap.innerHTML='<div class="modal-inner"><h3 data-i18n="modal_enter_code"></h3><input type="text" id="modalPairingCode" maxlength="6" inputmode="numeric" pattern="[0-9]*"/><div class="btns"><button id="modalPairingOk" data-i18n="modal_ok"></button><button id="modalPairingCancel" class="secondary" data-i18n="modal_cancel"></button></div></div>';
    document.body.appendChild(wrap);
    var codeIn=document.getElementById('modalPairingCode');
    codeIn.placeholder=G.PC.t('modal_ph');
    var ok=document.getElementById('modalPairingOk');
    var cancel=document.getElementById('modalPairingCancel');
    cancel.onclick=function(){ wrap.classList.remove('on'); };
    G.PC.requestWithCode=function(method,url,body,done){
      codeIn.value='';
      wrap.classList.add('on');
      function closeModal(){ wrap.classList.remove('on'); }
      function doReq(){
        var code=(codeIn.value||'').trim();
        if(code.length!==6||!/^\d+$/.test(code))return;
        closeModal();
        var csrfRetry=0;
        function send(){
          var opts={method:method,headers:{'X-Pairing-Code':code}};
          if(csrfToken) opts.headers['X-CSRF-Token']=csrfToken;
          if(body){ opts.headers['Content-Type']='application/json'; opts.body=body; }
          fetch(BASE+url,opts).then(function(r){
            return r.json().then(function(j){ return {ok:r.ok,j:j,status:r.status}; });
          }).then(function(x){
            if(!x.ok && x.status===403 && x.j && x.j.error && String(x.j.error).indexOf('CSRF')>=0 && csrfRetry<1){
              csrfRetry++;
              return fetch(BASE+'/api/csrf_token').then(function(r){ return r.json(); }).then(function(j){
                csrfToken=j.csrf_token||null;
                send();
              });
            }
            done({ok:x.ok,j:x.j});
          }).catch(function(){ done({ok:false,j:{error:G.PC.t('pairing_network')}}); });
        }
        function ensureCsrfThenSend(){
          if(csrfToken){ send(); return; }
          fetch(BASE+'/api/csrf_token').then(function(r){ return r.json(); }).then(function(j){
            csrfToken=j.csrf_token||null;
            send();
          }).catch(function(){ done({ok:false,j:{error:G.PC.t('pairing_network')}}); });
        }
        ensureCsrfThenSend();
      }
      ok.onclick=doReq;
    };
  }
  function subtitle(){
    var s=document.body.dataset.subtitle;
    if(s)return s;
    var t=document.title;
    return t.indexOf(' - ')>0?t.split(' - ')[0].trim():'';
  }
  function applyT(){
    document.querySelectorAll('[data-i18n]').forEach(function(el){
      var k=el.getAttribute('data-i18n');
      if(k)el.textContent=G.PC.t(k);
    });
    document.querySelectorAll('[data-i18n-ph]').forEach(function(el){
      var k=el.getAttribute('data-i18n-ph');
      if(k)el.placeholder=G.PC.t(k);
    });
  }
  function renderHeaderFooter(){
    if(document.getElementById('app-header'))return;
    var h=document.createElement('header');
    h.className='app-header';
    h.id='app-header';
    h.innerHTML='<h1>beetle</h1><p></p>';
    h.querySelector('p').textContent=subtitle();
    document.body.insertBefore(h,document.body.firstChild);
    var f=document.createElement('footer');
    f.className='app-footer';
    f.textContent='beetle';
    document.body.appendChild(f);
  }
  function renderInfoList(container,items,data){
    container.innerHTML='';
    items.forEach(function(it){
      var li=document.createElement('li');
      var lbl=document.createElement('span');
      lbl.className='label';
      lbl.textContent=G.PC.t(it.labelKey);
      var val=document.createElement('span');
      val.className='value';
      val.textContent=(data&&data[it.key])||'—';
      li.appendChild(lbl);
      li.appendChild(val);
      container.appendChild(li);
    });
  }
  G.PC={
    BASE:BASE,
    locale:'zh',
    T:T,
    t:function(k){ return (T[this.locale]&&T[this.locale][k])||T.zh[k]||k; },
    setLocale:function(l){ this.locale=(l==='en'?'en':'zh'); document.documentElement.lang=this.locale==='en'?'en':'zh-CN'; if(this.applyT)this.applyT(); },
    applyT:applyT,
    showMsg:showMsg,
    renderHeaderFooter:renderHeaderFooter,
    renderInfoList:renderInfoList
  };
  renderHeaderFooter();
  var path=window.location.pathname.replace(/\/$/,'')||'/wifi';
  var nav=document.getElementById('app-nav');
  if(nav){
    [{path:'/wifi',key:'nav_wifi'}].forEach(function(it){
      var a=document.createElement('a');
      a.href=it.path;
      a.setAttribute('data-i18n',it.key);
      a.textContent=G.PC.t(it.key);
      if(it.path===path)a.className='active';
      nav.appendChild(a);
    });
    createPairingModal();
    fetch(BASE+'/api/csrf_token').then(function(r){ return r.json(); }).then(function(j){ csrfToken=j.csrf_token||null; }).catch(function(){});
  }
  applyT();
})();
