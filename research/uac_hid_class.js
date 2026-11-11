height:e}=this.main.getContentBounds();
this.uadLabsWebContentsView.setBounds({
x:215,y:36,width:t-215,height:e-36})}}async captureUadLabsViewSnapshot(){
if(this.uadLabsWebContentsView)try{
return(await this.uadLabsWebContentsView.webContents.capturePage()).toDataURL()}catch(t){
return void this.log.error("uad labs: failed to capture a view snapshot:",t)}else this.log.debug("uad labs: cannot get a snapshot, web view not created yet")}authTokenUpdated(t){
t?(this.log.debug("uad labs: authentication token has been updated reloading the web view"),this.reloadUadLabsView()):(this.log.debug("uad labs: authentication token cleared, destroying the web view"),this.destroyUadLabsView())}reloadUadLabsView(){
this.uadLabsWebContentsView&&(this.log.debug("uad labs: reloading the web view"),this.uadLabsWebContentsView.webContents.reload())}isOnboardingInstallPending(){
return this.uacOnboardingHelper.isInstallPending()}isOnboardingUrlReady(){
return Ct(this.uacOnboardingHelper.getUrl())}createUACOnboardingView(t){
const e=this.uacOnboardingHelper.getUrl();
if(Pt(e))return this.log.warn("uac onboarding: no URL available, skipping view creation"),!1;
if(!this.main)throw new Error("Main window not found when creating UAC Onboarding WebContentsView");
if(this.uacOnboardingWebContentsView)return this.log.debug("uac onboarding: web view already exists"),!0;
this.uacOnboardingDeviceContext=t;
const s="uac_onboarding",r=new Set(["media","audioCapture","speaker-selection"]),n=i.session.fromPartition(s);
n.setPermissionCheckHandler((t,e)=>r.has(e)),n.setPermissionRequestHandler((t,e,i)=>{
i(r.has(e))});
const o={
contextIsolation:!0,nodeIntegration:!1,partition:s,preload:a.join(__dirname,"uacOnboardingPreload.cjs"),backgroundThrottling:!1},d=new i.WebContentsView({
webPreferences:o});
return d.setVisible(!1),this.uacOnboardingReadyGate=new to(this.log),d.webContents.on("did-fail-load",(t,e,i,s,r)=>{
var a;
(function(t,e){
return e&&-3!==t})(e,r)&&(this.log.error(`uac onboarding: wizard failed to load (${
e} ${
i})`),null==(a=this.uacOnboardingReadyGate)||a.fail())}),d.webContents.setWindowOpenHandler(({
url:t})=>(this.log.info("uac onboarding open external link",t),gn(t,this.log),{
action:"deny"})),this.fkOpenUACOnboardingDevTools&&d.webContents.openDevTools({
mode:"detach"}),d.webContents.loadURL(e),this.uacOnboardingWebContentsView=d,this.main.contentView.addChildView(d),this.uacOnboardingResize=()=>this.updateUACOnboardingBounds(),this.main.on("resize",this.uacOnboardingResize),this.updateUACOnboardingBounds(),this.sendToWindow("main","uac-onboarding:active",!0),!0}destroyUACOnboardingView(){
var t;
if(this.log.debug("uac onboarding: destroying the web view"),this.uacOnboardingWebContentsView){
this.main&&(this.main.contentView.removeChildView(this.uacOnboardingWebContentsView),this.uacOnboardingResize&&this.main.removeListener("resize",this.uacOnboardingResize)),this.uacOnboardingResize=void 0;
const t=this.uacOnboardingWebContentsView.webContents;
t&&!t.isDestroyed()&&t.close(),this.uacOnboardingWebContentsView=void 0,this.sendToWindow("main","uac-onboarding:active",!1)}null==(t=this.uacOnboardingReadyGate)||t.dispose(),this.uacOnboardingReadyGate=void 0,this.uacOnboardingDeviceContext=void 0}markUACOnboardingViewReady(){
var t;
null==(t=this.uacOnboardingReadyGate)||t.ready()}async setUACOnboardingViewVisible(t){
var e,i;
const s=this.uacOnboardingWebContentsView;
if(!s)return!1;
if(t){
if(await(null==(e=this.uacOnboardingReadyGate)?void 0:e.whenReady),this.uacOnboardingWebContentsView!==s)return!1;
if(null==(i=this.uacOnboardingReadyGate)?void 0:i.isFailed())return!1}return s.getVisible()!=t&&(this.log.debug("uac onboarding: set visibility to",t),s.setVisible(t)),!0}async captureUACOnboardingViewSnapshot(){
const t=this.uacOnboardingWebContentsView;
if(t&&t.getVisible())try{
return(await t.webContents.capturePage()).toDataURL()}catch(e){
return void this.log.error("uac onboarding: failed to capture a view snapshot",e)}}getUACOnboardingDeviceContext(){
return this.uacOnboardingDeviceContext}setUACOnboardingDeviceConnected(t,e){
var i;
const{
serialNumber:s,vendorId:r,productId:a}=t;
if(!(Ii(r,a)||Si(r,a)))return;
const n=this.getUACOnboardingDeviceConnected();
if(this.uacOnboardingDeviceContext&&(e&&!n&&(this.uacOnboardingDeviceContext.serialNumber=s),this.uacOnboardingDeviceContext.connected=e),n===e)return;
const o=null==(i=this.uacOnboardingWebContentsView)?void 0:i.webContents;
o&&(this.log.debug("uac onboarding: device connection changed",e),_a(o,"uac-onboarding:device-connection-changed",e))}getUACOnboardingDeviceConnected(){
var t;
return(null==(t=this.uacOnboardingDeviceContext)?void 0:t.connected)??!1}updateUACOnboardingBounds(){
if(this.uacOnboardingWebContentsView&&this.main){
const{
width:t,height:e}=this.main.getContentBounds();
this.uacOnboardingWebContentsView.setBounds({
x:0,y:0,width:t,height:e})}}}class ho extends co{
constructor(t){
super(t),e(this,"shouldQuitWhenMainWindowIsClosed",!1),e(this,"trayIcon"),e(this,"tray"),e(this,"trayState","closed"),this.trayIcon=t.trayIcon,this.splashWindowLaunchState=this.mainWindowLaunchState=t.windowHidden?"closed":"open"}setup(){
super.setup(),this.persist.getAutoUpdateState().then(t=>(t&&(this.mainWindowLaunchState=t.mainWindow,this.splashWindowLaunchState="closed"),this.persist.clearAutoUpdateState())).catch(t=>{
this.log.error("error checking auto-update window state",gt(t))})}async start(){
await super.start(),this.tray=await this.createTray()}buildTrayMenuTemplate(){
const t=[{
label:`Open ${
this.app.name}`,click:async()=>{
await this.manageWindows("open-main")}},{
type:"separator"},{
label:`Quit ${
this.app.name}`,click:async()=>{
this.isQuitting=!0,this.app.quit()}}];
return this.fkAllowTriggerUpdate&&(t.push({
type:"separator"}),t.push({
label:"Check for Updates...",click:async()=>{
this.onCheckForUpdates()}})),this.fkSimulatedDevices&&(t.push({
type:"separator"}),t.push({
label:"Open Simulated Devices",click:async()=>{
await this.manageWindows("open-simulation")}})),this.uadLabsHelper.isTrayMenuItemAllowed()&&this.getAuthToken()&&(t.push({
type:"separator"}),t.push({
label:"UAD Labs",click:async()=>{
await this.manageWindows("open-uadLabs")}})),this.fkFeedbackWindow&&(t.push({
type:"separator"}),t.push({
label:"Leave Feedback",click:async()=>{
await this.manageWindows("open-feedback")}})),t}refreshTrayContextMenu(){
if(!this.tray)return;
const t=i.Menu.buildFromTemplate(this.buildTrayMenuTemplate());
this.tray.setContextMenu(t)}featureKeysUpdated(t){
super.featureKeysUpdated(t),this.refreshTrayContextMenu()}authTokenUpdated(t){
super.authTokenUpdated(t),this.refreshTrayContextMenu()}async createTray(){
await this.app.whenReady();
const t=new i.Tray(this.trayIcon);
this.trayState="closed";
const e=i.Menu.buildFromTemplate(this.buildTrayMenuTemplate());
return"win32"===r.platform()&&t.on("click",async()=>{
await this.manageWindows("open-main")}),t.setContextMenu(e),t.setToolTip(this.app.name),t}async manageWindows(t){
var e,i,s,r;
switch(this.log.info(`window event "${
t}"`),t){
case"start":if("closed"===this.splashWindowLaunchState)return;
this.splash=await this.createSplashWindow(),await this.startSplashProgress(this.splash),this.log.debug("Splash progress done");
break;
case"ready":if("closed"===this.mainWindowLaunchState)return;
this.main=await this.ensureWindowExists("main"),"minimized"===this.mainWindowLaunchState&&this.main.isMinimizable()&&this.main.minimize(),await this.showMain(this.mainWindowLaunchState),this.afterMainWindowOpened(),this.whenReady.resolve();
break;
case"open-main":await this.showMain("open"),this.onScheduleCheckForUpdates(),this.afterMainWindowOpened();
break;
case"open-simulation":this.simulationControl=await this.ensureWindowExists("simulation"),this.positionSimulationControl(),null==(e=this.simulationControl)||e.show();
break;
case"close-main":null==(i=this.main)||i.close(),this.afterMainWindowClosed();
break;
case"open-feedback":this.feedback=await this.ensureWindowExists("feedback"),null==(s=this.feedback)||s.show();
break;
case"close-feedback":null==(r=this.feedback)||r.close();
break;
case"open-uadLabs":this.fkUadLabsExternalWindow?(this.createUadLabsView(),this.setUadLabsViewVisible(!0)):this.onNavigateMain({
version:ra,path:"uad-labs",params:{
}});
break;
default:throw new lt(`Unknown window event "${
t}`,"ERR_UNKNOWN_EVENT")}}}const po=Ds.BUILD_NUMBER??"1",mo=!tt(Ds.p),wo=!!Ds.m&&!tt(Ds.m),fo=!!Ds.S&&Ds.S.length>0,go="uaconnect";
function _o(t,e){
return t.getProperty(e)}class vo{
constructor(t){
e(this,"state"),e(this,"clients"),e(this,"syncClients"),e(this,"isSignedIn"),e(this,"_loadData"),e(this,"syncHandlers"),e(this,"propertyHandlers"),e(this,"defaultPropertyHandler"),e(this,"_name"),e(this,"log",ss),this.state={
},this.clients=t.clients,this.syncClients=t.syncClients??this.clients,this.isSignedIn=t.isSignedIn,this._loadData=t.loadData,this.syncHandlers=t.syncHandlers??{
},this.propertyHandlers=t.propertyHandlers??{
},this.defaultPropertyHandler=t.defaultPropertyHandler,this._name=t.name,this.log=t.log??ss}get name(){
return this._name}setup(){
var t;
const e=Object.keys(this.propertyHandlers),i=Object.keys(this.syncHandlers),s=e.filter(t=>i.includes(t));
if(s.length>0){
const t=s.join(", ");
this.log.warn(`${
this.name} duplicate handlers for [${
t}]. Property handlers will never fire.`)}for(const r of this.clients)r.addListener((t,e)=>{
var i,s,a;
"data"===e?this.propertyHandlers.data??this.handleData():e in this.syncHandlers?this.handleSyncing(r,e):e in this.propertyHandlers?null==(s=(i=this.propertyHandlers)[e])||s.call(i,{
name:t,property:e,clientName:r.name}):this.defaultPropertyHandler&&(null==(a=this.defaultPropertyHandler)||a.call(this,{
name:t,property:e,clientName:r.name}))});
for(const r of this.syncClients)for(const e of Object.values(this.syncHandlers))null==(t=e.onStarted)||t.call(e,r.name)}isSyncingComplete(){
return t=this.syncClients,e=Object.keys(this.syncHandlers),t.every(t=>0===e.length||e.every(e=>fs(_o(t,e))));
var t,e}loadData(){
this._loadData()}handleSyncing(t,e){
var i,s;
if(!this.isSignedIn())return;
const r=_o(t,e);
if(this.debug(`[sync] ${
t.name}/${
e}=${
r}`),function(t){
return"boolean"==typeof t?t:"syncing"===t}(r)){
const s=null==(i=this.syncHandlers[e])?void 0:i.onStarted;
null==s||s(t.name)}else if(fs(r)){
const i=null==(s=this.syncHandlers[e])?void 0:s.onCompleted;
i?i(t.name):t.refresh()}else this.debug(`[sync] ${
t.name} handleSyncing no callback fired`)}handleData(){
this.isSignedIn()&&this.loadData()}debug(...t){
this.log.debug(`[${
this.name}]`,...t)}}function yo(t,e,i){
return t in i&&typeof e==typeof i[t]}const Eo=class t{
constructor(t={
}){
e(this,"gui"),e(this,"resolveLogLevelSet"),e(this,"whenLogLevelSet",new Promise(t=>{
this.resolveLogLevelSet=t})),null!=t.onError&&A.catchErrors({
showDialog:t.showDialogOnError??!1,onError(e){
var i;
null==(i=t.onError)||i.call(t,e,A)}}),this.gui=this.scope("gui")}static create(e={
}){
return this.instance=new t(e),this.instance}static getInstance(){
return this.instance}getLog(){
return A}scope(t){
return A.scope(t)}setLogPath(t){
const e=`${
t.name}.log`;
A.transports.file.fileName=e,A.transports.file.resolvePath=function(i){
return G.join(t.getPath("logs"),i.fileName??e)}}setLogLevel(t){
var e;
for(const i of[A])i.transports.file.level=t,i.transports.console.level=t,i.transports.ipc&&(i.transports.ipc.level=t);
A.debug(`set log level to "${
t}"`),null==(e=this.resolveLogLevelSet)||e.call(this,t)}async logMessage(t,...e){
(this.gui[t]??this.gui.info)(...e)}};
e(Eo,"instance");
let bo=Eo;
const Ro="legacy";
class Ao{
constructor(t){
this.config=t}async getToolVersion(){
return"N/A"}async install(t){
const e=this.config.getDeviceState(t.device.serialNumber),i=this.config.products.findByUsb(t.device);
if(!e)throw new Error(`Simulated firmware update failed, missing device state for ${
t.device.serialNumber}`);
if(!i)throw new Error(`Simulated firmware update failed, missing product for ${
t.device.productId}`);
return await this.config.simulateFirmwareUpdate({
...t,...e,product:i}),pe}}class No{
constructor(t){
this.config=t}async open(){
return!0}isOpen(t){
return!0}async getUuid(t){
return Qa(t)}async getFwVersion(t){
return"0.99"}async getDeviceInfo(t){
return{
uuid:await this.getUuid(t),firmwareVersion:await this.getFwVersion(t)}}async close(t){
return!0}async closeAll(){
return!0}async reboot(t){
var e;
const{
device:i}=t;
let s=this.config.getDeviceState(i.serialNumber);
if(null==(e=this.config.log)||e.debug("reboot",t.device.serialNumber),!s)throw new Error(`Simulated reboot failed, missing device state for ${
i.serialNumber}`);
return this.config.simulateReboot({
...s,device:{
...i,serialNumber:s.serialNumber}})}async needsReboot(t){
return!1}async registered(t){
return!0}}class Co{
constructor(t){
e(this,"rebootDuration",500),e(this,"updateDuration",5e3),this.config=t}async simulateDeviceDisconnected(t){
const e=this.config.getDeviceState(t.serialNumber);
if(!e)throw new Error(`Error simulating device disconnected, no device state for ${
t.serialNumber}`);
va("device:disconnected",Ba(e.serialNumber,e.behaviors??{
}))}async simulateDeviceConnected(t){
var e;
const{
device:i,firmwareVersion:s,behaviors:r,product:a,registered:n,firmwareVersionFpga:o,firmwareVersionDsp:d}=t;
n&&(await this.simulateRegisterDevice(t),await Xi(1e3));
const l=Ba(i.serialNumber,r);
this.config.setDeviceState(i.serialNumber,{
serialNumber:i.serialNumber,product:a,firmwareVersion:s,firmwareVersionFpga:o,firmwareVersionDsp:d,behaviors:r,bootupBehaviors:this.createBootupBehaviors(r)}),null==(e=this.config.log)||e.debug("sending device:connected"),va("device:connected",{
device:{
...t.device,serialNumber:l},firmwareVersion:s,simulated:!0,firmwareVersionFpga:o,firmwareVersionDsp:d})}async simulateDeviceReady(t){
var e;
const{
device:i}=t,s=this.config.getDeviceState(i.serialNumber);
if(!s)throw new Error(`Error simulating device:ready, missing device for SN: ${
i.serialNumber}`);
const{
behaviors:r}=s;
if(r.neverBecomesReady)return void(null==(e=this.config.log)||e.debug("device never becomes ready",{
serialNumber:i.serialNumber}));
const a=await this.config.getFirmwareVersion(t.device.serialNumber),n=this.config.getFirmwareVersionFpga(t.device.serialNumber),o=this.config.getFirmwareVersionDsp(t.device.serialNumber),d=Ba(i.serialNumber,r),l=await this.config.getUuid(i.serialNumber);
va("device:ready",{
device:{
...t.device,serialNumber:d},firmwareVersion:a,uuid:l,simulated:!0,firmwareVersionFpga:n,firmwareVersionDsp:o})}createBootupBehaviors(t){
return t}newBehaviorsAfterFirmwareUpdate(t){
return t.behaviors}async simulateFirmwareUpdate(t){
var e,i,s;
const{
device:r,version:a,product:n}=t,o=this.config.getDeviceState(r.serialNumber);
if(!o)throw new Error(`Error simulating firmware update, missing device for SN: ${
r.serialNumber}`);
const{
behaviors:d}=o,l=this.createBootupBehaviors(d);
this.config.updateDeviceState(r.serialNumber,{
bootupBehaviors:l}),delete r.operation,delete r.operationStatus,await this.simulateReboot(t),await new Promise(e=>{
const i=this.updateDuration/100,s=a=>{
t.onProgress({
serialNumber:r.serialNumber,progress:a,secondsRemaining:(this.updateDuration-i*a)/1e3}),a<100?setTimeout(()=>s(a+1),i):e()};
s(0)}),this.config.setFirmwareVersion(r.serialNumber,a);
const u=this.newBehaviorsAfterFirmwareUpdate({
behaviors:d,productName:n.productName,firmwareVersion:a});
this.config.updateDeviceState(r.serialNumber,{
bootupBehaviors:u}),null==(e=this.config.log)||e.debug("after firmware update",{
behaviors:d,newBehaviors:u,state:this.config.getDeviceState(r.serialNumber)});
!(null==(i=l.invalidSerialAllF)?void 0:i.active)?await this.simulateReboot(t):null==(s=this.config.log)||s.debug("Skipping normal reboot after firmware update"),t.onStatus({
serialNumber:r.serialNumber,status:"done",firmwareVersion:a})}async simulateReboot(t){
var e,i;
null==(e=this.config.log)||e.debug("simulateReboot",t.device.serialNumber);
const s=t.delay??this.rebootDuration,{
device:r}=t,a=this.config.getDeviceState(r.serialNumber);
if(!a)throw new Error(`Error simulating reboot, missing device for SN: ${
r.serialNumber}`);
null==(i=this.config.log)||i.debug("simulateReboot device state",a);
const{
bootupBehaviors:n}=a;
await this.simulateDeviceDisconnected({
...t,serialNumber:r.serialNumber}),await new Promise(t=>setTimeout(t,s));
const o={
...t,device:r,behaviors:n,firmwareVersion:a.firmwareVersion};
await this.simulateDeviceConnected(o),this.config.updateDeviceState(r.serialNumber,{
behaviors:n}),await new Promise(t=>setTimeout(t,s)),await this.simulateDeviceReady(o)}async simulateRegisterDevice(t){
var e;
const i=await this.config.getUuid(t.device.serialNumber);
null==(e=this.config.log)||e.debug("sending simulated-device:register-usb"),va("simulated-device:register-usb",{
device:t.device,simulated:!0,uuid:i})}}class Po{
constructor(t){
var i;
e(this,"deviceData",{
}),e(this,"products",new Oi),e(this,"firmwareUpdateCallbacks"),e(this,"deviceCallbacks"),e(this,"controlCallbacks"),e(this,"firmware"),e(this,"driver"),e(this,"control"),e(this,"device"),this.config=t,this.firmwareUpdateCallbacks={
getFirmwareVersion:t=>{
var e;
return(null==(e=this.getDeviceState(t))?void 0:e.firmwareVersion)??""},getDeviceState:t=>this.getDeviceState(t),updateDeviceState:(t,e)=>this.updateDeviceState(t,e),simulateFirmwareUpdate:t=>this.control.simulateFirmwareUpdate(t),products:this.products,log:this.config.log},this.deviceCallbacks={
getDeviceState:t=>this.getDeviceState(t),simulateReboot:t=>this.control.simulateReboot(t),log:this.config.log},this.controlCallbacks={
getFirmwareVersion:t=>{
var e;
return(null==(e=this.getDeviceState(t))?void 0:e.firmwareVersion)??""},setFirmwareVersion:(t,e)=>this.updateDeviceState(t,{
firmwareVersion:e}),getDeviceState:t=>this.getDeviceState(t),setDeviceState:(t,e)=>this.setDeviceState(t,e),updateDeviceState:(t,e)=>this.updateDeviceState(t,e),getUuid:t=>this.device.getUuid(t),getFirmwareVersionFpga:t=>{
var e;
return null==(e=this.getDeviceState(t))?void 0:e.firmwareVersionFpga},getFirmwareVersionDsp:t=>{
var e;
return null==(e=this.getDeviceState(t))?void 0:e.firmwareVersionDsp},log:this.config.log},this.firmware=t.createFirmwareTools(this.firmwareUpdateCallbacks),this.control=t.createSimulationControl(this.controlCallbacks),this.driver=null==(i=t.createDriverTools)?void 0:i.call(t),this.device=t.createDeviceTools(this.deviceCallbacks)}getDeviceState(t){
return this.deviceData[t]??this.findDeviceStatePresentingAs(t)}setDeviceState(t,e){
var i;
null==(i=this.config.log)||i.debug("setDeviceState",{
serialNumber:t,data:e}),this.deviceData[t]=e}updateDeviceState(t,e){
var i;
null==(i=this.config.log)||i.debug("updateDeviceState",{
serialNumber:t,data:e}),this.deviceData[t]={
...this.deviceData[t],...e}}findDeviceStatePresentingAs(t){
var e;
const i=Object.values(this.deviceData).find(e=>t===Ba(e.serialNumber,e.behaviors));
return i||null==(e=this.config.log)||e.debug(JSON.stringify(this.deviceData,null,2)),i}}class To extends Po{
constructor(t){
super({
...t,createFirmwareTools:t=>new Ao(t),createDeviceTools:t=>new No(t),createSimulationControl:t=>new Co(t)})}}const Uo={
200:"OK",400:"BAD_REQUEST"};
function So(t){
const e=Uo[t];
if(null==e)throw new Error(`Invalid status code ${
t}`);
return{
code:+t,status:e}}function Do(t){
const{
active:e,exit_code:i,stderr:s,stdout:r}=t;
return{
active:e,exit_code:i,stderr:s,stdout:r}}function Io(t,e){
return new Promise(i=>{
setTimeout(()=>{
i(e)},t)})}class ko{
constructor(t){
e(this,"transport","sim"),e(this,"isConnected",!1),e(this,"uuid"),e(this,"updateTask",{
active:!1,exit_code:0,stderr:"",stdout:"",reboot:!0}),e(this,"listeners",new c.ConnectionListeners),e(this,"compatibleUpdates",[]),e(this,"incompatibleUpdates",[]),e(this,"lastCompat",{
input:"",isCompatible:!0}),e(this,"handleGetUpdateTask",async t=>{
const{
path:e,parameters:i}=t;
return{
path:e,parameters:i,data:{
properties:Do(this.updateTask)}}}),e(this,"handlePostUpdateTask",async t=>{
const{
path:e,parameters:i,data:s}=t,{
reboot:r}=s;
return null!=r&&(this.updateTask.reboot=!1),{
path:e,parameters:i,data:So(200)}}),e(this,"handlePostAbortUpdateTask",async t=>{
const{
path:e,parameters:i}=t;
return this.updateTask.active=!1,{
path:e,parameters:i,data:So(200)}}),e(this,"handlePostReboot",async t=>{
const{
path:e,parameters:i}=t;
return setTimeout(()=>{
this.config.onReboot(this)},3e3),{
path:e,parameters:i,data:So(200)}}),e(this,"handleGetSendSuccess",async t=>{
const{
path:e,parameters:i}=t;
return Io(10,{
path:e,parameters:i,data:{
properties:{
name:"send",description:"Send base64-encoded data to an active firmware-update.",active:!1,stderr:"",exit_code:0,stdout:""}}})}),e(this,"handlePostSendSuccess",async t=>{
const{
path:e,parameters:i}=t;
return{
path:e,parameters:i,data:So(200)}}),e(this,"handlePostFinalizeUpdate",async t=>{
const{
path:e,parameters:i}=t;
return{
path:e,parameters:i,data:So(200)}}),e(this,"handleSerialNumber",async t=>{
const{
path:e,parameters:i}=t;
return{
path:e,parameters:i,data:this.config.serialNumber}}),e(this,"handleRegistered",async t=>{
const{
path:e,parameters:i}=t;
return{
path:e,parameters:i,data:this.config.registered}}),e(this,"handleVersion",async t=>{
const{
path:e,parameters:i}=t;
return{
path:e,parameters:i,data:this.config.currentVersion}}),e(this,"handleCustomName",as